use std::{
    collections::HashMap,
    fmt,
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
};

#[cfg(target_arch = "wasm32")]
use std::collections::{HashSet, VecDeque};

#[cfg(target_arch = "wasm32")]
use std::cell::RefCell;

#[cfg(not(target_arch = "wasm32"))]
use std::fs;

use bevy::{
    asset::RenderAssetUsages,
    log::tracing::trace_span,
    log::*,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    tasks::IoTaskPool,
};
use image::{GenericImageView, ImageError};
use reqwest::{
    StatusCode,
    header::{CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue},
};
use tilemath::Tile as TileMathTile;

use crate::Tile;

#[cfg(target_arch = "wasm32")]
use indexed_db_futures::{
    database::Database,
    prelude::*,
    transaction::TransactionMode,
    typed_array::Uint8Array,
};

#[cfg(not(target_arch = "wasm32"))]
use reqwest::blocking::Client;

#[cfg(target_arch = "wasm32")]
use reqwest::Client;

/// Configuration for downloading map tiles.
#[derive(Resource, Clone, Debug)]
pub struct TileFetchConfig {
    /// Template URL that contains `{z}`, `{x}`, and `{y}` placeholders.
    pub url_template: String,
    /// Optional HTTP headers sent with every tile request.
    pub headers: HashMap<String, String>,
    /// Directory used to cache downloaded tiles on disk.
    pub cache_directory: PathBuf,
    /// File extension used when caching tiles locally (defaults to `png`).
    pub cache_extension: String,
    /// Whether to use TMS-style Y coordinates (origin bottom-left) instead of XYZ-style (origin top-left).
    pub reverse_y: bool,
    /// zoom level offset applied when fetching tiles (can be negative).
    /// For example, with an offset of -1, tile 3/4/2 will be fetched when tile 4/4/2 is requested.
    pub zoom_offset: i8,
}

impl Default for TileFetchConfig {
    fn default() -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        let mut headers = HashMap::new();
        #[cfg(not(target_arch = "wasm32"))]
        headers.insert("User-Agent".to_string(), "bevy-geo-tiles/0.1".to_string());
        #[cfg(target_arch = "wasm32")]
        let headers = HashMap::new();
        Self {
            url_template: "https://tile.openstreetmap.org/{z}/{x}/{y}.png".to_string(),
            headers,
            cache_directory: default_cache_dir(),
            cache_extension: "png".to_string(),
            reverse_y: false,
            zoom_offset: 0,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn default_cache_dir() -> PathBuf {
    std::env::var("BEVY_GEO_TILES_CACHE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("bevy_geo_tiles_cache"))
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn default_cache_dir() -> PathBuf {
    PathBuf::new()
}

/// Error type for tile fetching operations.
#[derive(Debug)]
pub enum TileFetchError {
    HttpStatus(StatusCode),
    Network(String),
    Io(String),
    Decode(String),
}

impl fmt::Display for TileFetchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TileFetchError::HttpStatus(code) => write!(f, "HTTP request failed with status {code}"),
            TileFetchError::Network(err) => write!(f, "network error: {err}"),
            TileFetchError::Io(err) => write!(f, "io error: {err}"),
            TileFetchError::Decode(err) => write!(f, "decode error: {err}"),
        }
    }
}

impl std::error::Error for TileFetchError {}

impl TileFetchError {
    fn from_network(err: reqwest::Error) -> Self {
        Self::Network(err.to_string())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn from_io(err: std::io::Error) -> Self {
        Self::Io(err.to_string())
    }

    fn from_decode(err: ImageError) -> Self {
        Self::Decode(err.to_string())
    }
}

#[derive(Debug)]
struct PreparedConfig {
    template: String,
    headers: Vec<(HeaderName, HeaderValue)>,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    cache_directory: PathBuf,
    cache_extension: String,
}

impl PreparedConfig {
    fn format_url(&self, tile: &TileMathTile) -> String {
        self.template
            .replace("{z}", &tile.zoom.to_string())
            .replace("{x}", &tile.x.to_string())
            .replace("{y}", &tile.y.to_string())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn cache_path(&self, tile: &TileMathTile) -> PathBuf {
        let mut path = self.cache_directory.clone();
        path.push(tile.zoom.to_string());
        path.push(tile.x.to_string());
        path.push(format!("{}.{}", tile.y, self.cache_extension));
        path
    }
}

#[derive(Debug)]
struct TileImagePayload {
    bytes: Vec<u8>,
    cached_path: Option<PathBuf>,
    content_type: Option<String>,
    from_cache: bool,
}

#[derive(Resource, Debug)]
pub struct TileFetcher {
    client: Arc<Client>,
    config: Arc<PreparedConfig>,
    sender: mpsc::Sender<(TileMathTile, Result<TileImagePayload, TileFetchError>)>,
    receiver: Arc<Mutex<mpsc::Receiver<(TileMathTile, Result<TileImagePayload, TileFetchError>)>>>,
    waiting: HashMap<TileMathTile, Vec<Entity>>,
    #[cfg(target_arch = "wasm32")]
    lru: Arc<Mutex<CacheLru>>,
}

impl FromWorld for TileFetcher {
    fn from_world(world: &mut World) -> Self {
        let config = world
            .get_resource::<TileFetchConfig>()
            .cloned()
            .unwrap_or_default();
        TileFetcher::new(config).expect("failed to construct TileFetcher")
    }
}

impl TileFetcher {
    pub fn new(config: TileFetchConfig) -> Result<Self, TileFetchError> {
        let mut default_headers = HeaderMap::new();
        let mut prepared_headers = Vec::new();
        for (name, value) in &config.headers {
            #[cfg(target_arch = "wasm32")]
            if name.eq_ignore_ascii_case("user-agent") {
                continue;
            }
            let header_name = HeaderName::from_bytes(name.as_bytes())
                .map_err(|err| TileFetchError::Network(err.to_string()))?;
            let header_value = HeaderValue::from_str(value)
                .map_err(|err| TileFetchError::Network(err.to_string()))?;
            default_headers.insert(header_name.clone(), header_value.clone());
            prepared_headers.push((header_name, header_value));
        }

        let client = Client::builder()
            .default_headers(default_headers.clone())
            .build()
            .map_err(TileFetchError::from_network)?;

        let prepared = PreparedConfig {
            template: config.url_template.clone(),
            headers: prepared_headers,
            cache_directory: config.cache_directory,
            cache_extension: config.cache_extension,
        };

        #[cfg(target_arch = "wasm32")]
        let lru = CacheLru::seed_from_storage();

        #[cfg(not(target_arch = "wasm32"))]
        if !prepared.cache_directory.exists() {
            fs::create_dir_all(&prepared.cache_directory).map_err(TileFetchError::from_io)?;
        }

        let (sender, receiver) = mpsc::channel();

        Ok(Self {
            client: Arc::new(client),
            config: Arc::new(prepared),
            sender,
            receiver: Arc::new(Mutex::new(receiver)),
            waiting: HashMap::new(),
            #[cfg(target_arch = "wasm32")]
            lru,
        })
    }

    pub fn request_tile(&mut self, entity: Entity, tile: TileMathTile) {
        let entry = self.waiting.entry(tile).or_default();
        if !entry.contains(&entity) {
            entry.push(entity);
        }

        if entry.len() > 1 {
            return;
        }

        let client = Arc::clone(&self.client);
        let sender = self.sender.clone();
        let config = Arc::clone(&self.config);
        #[cfg(target_arch = "wasm32")]
        let lru = Arc::clone(&self.lru);

        IoTaskPool::get()
            .spawn(async move {
                #[cfg(target_arch = "wasm32")]
                let result = fetch_tile(config, client, tile, lru).await;
                #[cfg(not(target_arch = "wasm32"))]
                let result = fetch_tile(config, client, tile);
                let _ = sender.send((tile, result));
            })
            .detach();
    }

    fn drain_ready(
        &mut self,
    ) -> Vec<(
        Vec<Entity>,
        TileMathTile,
        Result<TileImagePayload, TileFetchError>,
    )> {
        let mut responses = Vec::new();
        loop {
            let _span_once = info_span!("drain_ready_once", name = "drain_ready_once").entered();
            let message = {
                let receiver = self.receiver.lock().unwrap();
                receiver.try_recv()
            };

            match message {
                Ok((tile, result)) => {
                    if let Some(listeners) = self.waiting.remove(&tile) {
                        responses.push((listeners, tile, result));
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => break,
            }
        }
        responses
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn fetch_tile(
    config: Arc<PreparedConfig>,
    client: Arc<Client>,
    tile: TileMathTile,
) -> Result<TileImagePayload, TileFetchError> {
    let cache_path = config.cache_path(&tile);
    if cache_path.exists() {
        debug!("loading cached tile (x={}, y={})", tile.x, tile.y);
        let data = fs::read(&cache_path).map_err(TileFetchError::from_io)?;
        return Ok(TileImagePayload {
            bytes: data,
            cached_path: Some(cache_path),
            content_type: None,
            from_cache: true,
        });
    }
    debug!("fetching tile (x={}, y={})", tile.x, tile.y);
    let mut request = client.get(config.format_url(&tile));
    for (name, value) in &config.headers {
        request = request.header(name.clone(), value.clone());
    }

    let response = request.send().map_err(TileFetchError::from_network)?;
    if !response.status().is_success() {
        return Err(TileFetchError::HttpStatus(response.status()));
    }

    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value: &HeaderValue| value.to_str().ok())
        .map(str::to_owned);
    let bytes = response
        .bytes()
        .map_err(TileFetchError::from_network)?
        .to_vec();

    if let Some(parent) = cache_path.parent() {
        fs::create_dir_all(parent).map_err(TileFetchError::from_io)?;
    }
    fs::write(&cache_path, &bytes).map_err(TileFetchError::from_io)?;

    Ok(TileImagePayload {
        bytes,
        cached_path: Some(cache_path),
        content_type,
        from_cache: false,
    })
}

#[cfg(target_arch = "wasm32")]
async fn fetch_tile(
    config: Arc<PreparedConfig>,
    client: Arc<Client>,
    tile: TileMathTile,
    lru: Arc<Mutex<CacheLru>>,
) -> Result<TileImagePayload, TileFetchError> {
    let cache_key = storage_cache_key(&config, &tile);
    if let Some(data) = read_local_tile(&cache_key, &lru).await? {
        debug!("loading cached tile from IndexedDB (x={}, y={})", tile.x, tile.y);
        return Ok(TileImagePayload {
            bytes: data,
            cached_path: None,
            content_type: None,
            from_cache: true,
        });
    }

    debug!("fetching tile (x={}, y={})", tile.x, tile.y);
    let mut request = client.get(config.format_url(&tile));
    for (name, value) in &config.headers {
        request = request.header(name.clone(), value.clone());
    }

    let response = request.send().await.map_err(TileFetchError::from_network)?;
    if !response.status().is_success() {
        return Err(TileFetchError::HttpStatus(response.status()));
    }

    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value: &HeaderValue| value.to_str().ok())
        .map(str::to_owned);
    let bytes = response
        .bytes()
        .await
        .map_err(TileFetchError::from_network)?
        .to_vec();

    if let Err(err) = write_local_tile(&cache_key, &bytes, &lru).await {
        warn!("failed to cache tile in IndexedDB (x={}, y={}): {}", tile.x, tile.y, err);
    }

    Ok(TileImagePayload {
        bytes,
        cached_path: None,
        content_type,
        from_cache: false,
    })
}

#[derive(Component, Debug, Default)]
pub struct TileTextureLoading;

#[derive(Component, Debug)]
pub struct TileTextureError {
    pub message: Arc<str>,
}

impl TileTextureError {
    fn new(message: impl Into<String>) -> Self {
        let boxed: Box<str> = message.into().into_boxed_str();
        Self {
            message: Arc::<str>::from(boxed),
        }
    }
}

pub fn queue_tile_downloads(
    mut commands: Commands,
    mut fetcher: ResMut<TileFetcher>,
    config: Res<TileFetchConfig>,
    tiles: Query<(Entity, &Tile), Added<Tile>>,
) {
    for (entity, tile) in tiles.iter() {
        // some tile-servers use
        let zoom = (tile.0.zoom as i8 + config.zoom_offset).max(0) as u8;
        // current logic is inverted, so we flip the condition
        let y = if !config.reverse_y {
            (1 << zoom) - 1 - tile.0.y
        } else {
            tile.0.y
        };
        fetcher.request_tile(
            entity,
            TileMathTile {
                zoom,
                x: tile.0.x,
                y,
            },
        );
        commands
            .entity(entity)
            .remove::<TileTextureError>()
            .insert(TileTextureLoading);
    }
}

pub fn apply_tile_fetch_results(
    mut commands: Commands,
    mut fetcher: ResMut<TileFetcher>,
    mut images: ResMut<Assets<Image>>,
) {
    let _span = trace_span!("apply_tile_fetch_results",).entered();
    for (entities, tile, result) in fetcher.drain_ready() {
        match result {
            Ok(payload) => {
                if payload.from_cache
                    && let Some(path) = &payload.cached_path
                {
                    trace!("tile {:?} served from cache {}", tile, path.display());
                }

                if let Some(content_type) = &payload.content_type {
                    trace!("tile {:?} reported content-type {}", tile, content_type);
                }

                match build_image_from_payload(&payload) {
                    Ok(image) => {
                        let handle = images.add(image);
                        for entity in &entities {
                            if let Ok(mut entity_commands) = commands.get_entity(*entity) {
                                entity_commands.remove::<TileTextureLoading>();
                                entity_commands
                                    .insert(Sprite {
                                        image: handle.clone(),
                                        custom_size: Some(Vec2::ONE),
                                        ..Default::default()
                                    })
                                    .remove::<TileTextureError>();
                            }
                        }
                    }
                    Err(err) => {
                        error!("failed to decode tile {:?}: {}", tile, err);
                        for entity in entities {
                            if let Ok(mut entity_commands) = commands.get_entity(entity) {
                                entity_commands.remove::<TileTextureLoading>();
                                entity_commands.insert(TileTextureError::new(err.to_string()));
                            }
                        }
                    }
                }
            }
            Err(err) => {
                error!("failed to fetch tile {:?}: {:?}", tile, err);
                for entity in entities {
                    if let Ok(mut entity_commands) = commands.get_entity(entity) {
                        entity_commands.remove::<TileTextureLoading>();
                        entity_commands
                            .insert(TileTextureError::new(format!("Download failed: {:?}", err)));
                    }
                }
            }
        }
    }
}

fn build_image_from_payload(payload: &TileImagePayload) -> Result<Image, TileFetchError> {
    let dynamic = image::load_from_memory(&payload.bytes).map_err(TileFetchError::from_decode)?;
    let rgba = dynamic.to_rgba8();
    let (width, height) = dynamic.dimensions();
    Ok(Image::new_fill(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &rgba,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    ))
}

#[cfg(target_arch = "wasm32")]
fn storage_cache_key(config: &PreparedConfig, tile: &TileMathTile) -> String {
    format!(
        "bevy_geo_tiles:{}/{}/{}.{}",
        tile.zoom, tile.x, tile.y, config.cache_extension
    )
}

#[cfg(target_arch = "wasm32")]
async fn read_local_tile(
    cache_key: &str,
    lru: &Arc<Mutex<CacheLru>>,
) -> Result<Option<Vec<u8>>, TileFetchError> {
    let db = open_tile_db().await?;
    let transaction = db
        .transaction(IDB_STORE)
        .with_mode(TransactionMode::Readonly)
        .build()
        .map_err(|err| TileFetchError::Io(format!("{err:?}")))?;
    let store = transaction
        .object_store(IDB_STORE)
        .map_err(|err| TileFetchError::Io(format!("{err:?}")))?;
    let request = store
        .get::<Uint8Array, _, _>(cache_key)
        .primitive()
        .map_err(|err| TileFetchError::Io(format!("{err:?}")))?;
    let result = request.await.map_err(|err| TileFetchError::Io(format!("{err:?}")))?;
    if let Some(bytes) = result {
        if let Ok(mut lru) = lru.lock() {
            lru.touch(cache_key.to_string());
        }
        return Ok(Some(bytes.into()));
    }
    Ok(None)
}

#[cfg(target_arch = "wasm32")]
async fn write_local_tile(
    cache_key: &str,
    bytes: &[u8],
    lru: &Arc<Mutex<CacheLru>>,
) -> Result<(), TileFetchError> {
    let db = open_tile_db().await?;
    if let Err(err) = put_tile_bytes(&db, cache_key, bytes).await {
        if !is_quota_exceeded(&err) {
            return Err(TileFetchError::Io(format!("{err:?}")));
        }

        loop {
            let oldest = if let Ok(mut lru) = lru.lock() {
                lru.pop_oldest()
            } else {
                None
            };

            match oldest {
                Some(oldest_key) => {
                    let _ = delete_tile_key(&db, &oldest_key).await;
                    match put_tile_bytes(&db, cache_key, bytes).await {
                        Ok(()) => {
                            if let Ok(mut lru) = lru.lock() {
                                lru.touch(cache_key.to_string());
                            }
                            return Ok(());
                        }
                        Err(retry_err) => {
                            if !is_quota_exceeded(&retry_err) {
                                return Err(TileFetchError::Io(format!("{retry_err:?}")));
                            }
                            continue;
                        }
                    }
                }
                None => return Err(TileFetchError::Io(format!("{err:?}"))),
            }
        }
    }

    if let Ok(mut lru) = lru.lock() {
        lru.touch(cache_key.to_string());
    }
    Ok(())
}

#[cfg(target_arch = "wasm32")]
async fn open_tile_db(
) -> Result<Database, TileFetchError> {
    if let Some(existing) = get_cached_db() {
        return Ok(existing);
    }

    let opened = Database::open(IDB_NAME)
        .with_version(IDB_VERSION)
        .with_on_upgrade_needed(|_event, db| {
            if !db.object_store_names().any(|name| name == IDB_STORE) {
                db.create_object_store(IDB_STORE).build()?;
            }
            Ok(())
        })
        .await
        .map_err(|err| TileFetchError::Io(format!("{err:?}")))?;

    set_cached_db(opened.clone());
    Ok(opened)
}

#[cfg(target_arch = "wasm32")]
async fn put_tile_bytes(
    db: &Database,
    cache_key: &str,
    bytes: &[u8],
) -> Result<(), indexed_db_futures::error::Error> {
    let transaction = db
        .transaction(IDB_STORE)
        .with_mode(TransactionMode::Readwrite)
        .build()?;
    let store = transaction.object_store(IDB_STORE)?;
    let request = store
        .put(Uint8Array::from(bytes.to_vec()))
        .with_key(cache_key)
        .primitive()?;
    request.await?;
    transaction.commit().await?;
    Ok(())
}

#[cfg(target_arch = "wasm32")]
async fn delete_tile_key(
    db: &Database,
    cache_key: &str,
) -> Result<(), indexed_db_futures::error::Error> {
    let transaction = db
        .transaction(IDB_STORE)
        .with_mode(TransactionMode::Readwrite)
        .build()?;
    let store = transaction.object_store(IDB_STORE)?;
    let request = store.delete::<String, _>(cache_key.to_string()).primitive()?;
    request.await?;
    transaction.commit().await?;
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn is_quota_exceeded(err: &impl fmt::Debug) -> bool {
    let text = format!("{err:?}").to_ascii_lowercase();
    text.contains("quotaexceeded") || text.contains("quota exceeded")
}

#[cfg(target_arch = "wasm32")]
#[derive(Debug, Default)]
struct CacheLru {
    order: VecDeque<String>,
    entries: HashSet<String>,
}

#[cfg(target_arch = "wasm32")]
impl CacheLru {
    fn seed_from_storage() -> Arc<Mutex<Self>> {
        let lru = Arc::new(Mutex::new(Self::default()));
        let lru_clone = Arc::clone(&lru);
        IoTaskPool::get()
            .spawn(async move {
                if let Ok(opened) = open_tile_db().await {
                    if let Ok(keys) = read_all_keys(&opened).await {
                        if let Ok(mut lru_guard) = lru_clone.lock() {
                            for key in keys {
                                lru_guard.entries.insert(key.clone());
                                lru_guard.order.push_back(key);
                            }
                        }
                    }
                }
            })
            .detach();
        lru
    }

    fn touch(&mut self, key: String) {
        if self.entries.contains(&key) {
            self.order.retain(|item| item != &key);
        } else {
            self.entries.insert(key.clone());
        }
        self.order.push_back(key);
    }

    fn pop_oldest(&mut self) -> Option<String> {
        let key = self.order.pop_front()?;
        self.entries.remove(&key);
        Some(key)
    }
}

#[cfg(target_arch = "wasm32")]
const IDB_NAME: &str = "bevy_geo_tiles";
#[cfg(target_arch = "wasm32")]
const IDB_STORE: &str = "tiles";
#[cfg(target_arch = "wasm32")]
const IDB_VERSION: u8 = 1;

#[cfg(target_arch = "wasm32")]
async fn read_all_keys(db: &Database) -> Result<Vec<String>, indexed_db_futures::error::Error> {
    let transaction = db
        .transaction(IDB_STORE)
        .with_mode(TransactionMode::Readonly)
        .build()?;
    let store = transaction.object_store(IDB_STORE)?;
    let request = store.get_all_keys::<String>().primitive()?;
    let keys: Vec<String> = request.await?.collect::<Result<Vec<_>, _>>()?;
    transaction.commit().await?;
    Ok(keys)
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    static TILE_DB: RefCell<Option<Database>> = RefCell::new(None);
}

#[cfg(target_arch = "wasm32")]
fn get_cached_db() -> Option<Database> {
    TILE_DB.with(|cell| cell.borrow().clone())
}

#[cfg(target_arch = "wasm32")]
fn set_cached_db(db: Database) {
    TILE_DB.with(|cell| {
        *cell.borrow_mut() = Some(db);
    });
}
