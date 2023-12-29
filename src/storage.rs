use crate::assets::Asset;
use crate::state::event::{Event, EventType};
use ic_stable_structures::{
    log::Log as StableLog,
    memory_manager::{MemoryId, MemoryManager, VirtualMemory},
    storable::Bound,
    storable::Storable,
    DefaultMemoryImpl, StableBTreeMap,
};
use std::borrow::Cow;
use std::cell::RefCell;

const LOG_INDEX_MEMORY_ID: MemoryId = MemoryId::new(0);
const LOG_DATA_MEMORY_ID: MemoryId = MemoryId::new(1);
const ASSETS_MEMORY_ID: MemoryId = MemoryId::new(2);

type VMem = VirtualMemory<DefaultMemoryImpl>;
type EventLog = StableLog<Event, VMem, VMem>;

impl Storable for Event {
    fn to_bytes(&self) -> Cow<[u8]> {
        let mut buf = vec![];
        minicbor::encode(self, &mut buf).expect("event encoding should always succeed");
        Cow::Owned(buf)
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        minicbor::decode(bytes.as_ref())
            .unwrap_or_else(|e| panic!("failed to decode event bytes {}: {e}", hex::encode(bytes)))
    }

    const BOUND: Bound = Bound::Unbounded;
}

impl Storable for Asset {
    fn to_bytes(&self) -> Cow<[u8]> {
        let mut buf = vec![];
        minicbor::encode(self, &mut buf).expect("asset encoding should always succeed");
        Cow::Owned(buf)
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        minicbor::decode(bytes.as_ref())
            .unwrap_or_else(|e| panic!("failed to decode asset bytes {}: {e}", hex::encode(bytes)))
    }

    const BOUND: Bound = Bound::Unbounded;
}

thread_local! {
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> = RefCell::new(
        MemoryManager::init(DefaultMemoryImpl::default())
    );

    /// The log of the ckETH state modifications.
    static EVENTS: RefCell<EventLog> = MEMORY_MANAGER
        .with(|m|
            RefCell::new(
                StableLog::init(
                    m.borrow().get(LOG_INDEX_MEMORY_ID),
                    m.borrow().get(LOG_DATA_MEMORY_ID)
                ).expect("failed to initialize stable log")
            )
        );

    // Initialize a `StableBTreeMap`
    static ASSETS : RefCell<StableBTreeMap<String, Asset, VMem>> = MEMORY_MANAGER
        .with(|m|
            RefCell::new(
                StableBTreeMap::init(
                    m.borrow().get(ASSETS_MEMORY_ID)
                )
            )
    );
}

/// Stores the asset in the stable memory.
pub fn store_asset(path: String, asset: Asset) {
    ASSETS.with(|assets| assets.borrow_mut().insert(path, asset));
}

/// Gets an assset from stable memory.
/// Returns `None` if the asset is not found.
/// Returns `Some(asset)` if the asset is found.
pub fn get_asset(path: &String) -> Option<Asset> {
    ASSETS.with(|assets| assets.borrow().get(path))
}

/// Appends the event to the event log.
pub fn record_event(payload: EventType) {
    EVENTS
        .with(|events| {
            events.borrow().append(&Event {
                timestamp: ic_cdk::api::time(),
                payload,
            })
        })
        .expect("recording an event should succeed");
}

/// Returns the total number of events in the audit log.
pub fn total_event_count() -> u64 {
    EVENTS.with(|events| events.borrow().len())
}

pub fn with_event_iter<F, R>(f: F) -> R
where
    F: for<'a> FnOnce(Box<dyn Iterator<Item = Event> + 'a>) -> R,
{
    EVENTS.with(|events| f(Box::new(events.borrow().iter())))
}
