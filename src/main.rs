use candid::candid_method;
use ic_canister_log::log;
use ic_canisters_http_types::{HttpRequest, HttpResponse, HttpResponseBuilder};
use ic_cdk_macros::{init, post_upgrade, pre_upgrade, query, update};

use ic_cketh_minter::deposit::scrape_eth_logs;
use ic_cketh_minter::endpoints::events::{
    Event as CandidEvent, EventSource as CandidEventSource, GetEventsArg, GetEventsResult,
};

use ic_cketh_minter::eth_logs::{EventSource, MintEvent};
use ic_cketh_minter::eth_rpc::into_nat;
use ic_cketh_minter::lifecycle::MinterArg;
use ic_cketh_minter::logs::INFO;

use ic_cketh_minter::state::audit::{Event, EventType};
use ic_cketh_minter::state::{read_state, State, STATE};
use ic_cketh_minter::{storage, SCRAPPING_ETH_LOGS_INTERVAL};

use std::time::Duration;

mod dashboard;
pub const SEPOLIA_TEST_CHAIN_ID: u64 = 11155111;

fn setup_timers() {
    // Start scraping logs immediately after the install, then repeat with the interval.
    ic_cdk_timers::set_timer(Duration::from_secs(0), || ic_cdk::spawn(scrape_eth_logs()));
    ic_cdk_timers::set_timer_interval(SCRAPPING_ETH_LOGS_INTERVAL, || {
        ic_cdk::spawn(scrape_eth_logs())
    });
}

#[init]
#[candid_method(init)]
fn init(arg: MinterArg) {
    match arg {
        MinterArg::InitArg(init_arg) => {
            log!(INFO, "[init]: initialized minter with arg: {:?}", init_arg);
            STATE.with(|cell| {
                storage::record_event(EventType::Init(init_arg.clone()));
                *cell.borrow_mut() =
                    Some(State::try_from(init_arg).expect("BUG: failed to initialize minter"))
            });
        }
        MinterArg::UpgradeArg(_) => {
            ic_cdk::trap("cannot init canister state with upgrade args");
        }
    }
    setup_timers();
}

fn emit_preupgrade_events() {
    read_state(|s| {
        storage::record_event(EventType::SyncedToBlock {
            block_number: s.last_scraped_block_number,
        });
    });
}

#[pre_upgrade]
fn pre_upgrade() {
    emit_preupgrade_events();
}

#[post_upgrade]
fn post_upgrade(minter_arg: Option<MinterArg>) {
    use ic_cketh_minter::lifecycle;
    match minter_arg {
        Some(MinterArg::InitArg(_)) => {
            ic_cdk::trap("cannot upgrade canister state with init args");
        }
        Some(MinterArg::UpgradeArg(upgrade_args)) => lifecycle::post_upgrade(Some(upgrade_args)),
        None => lifecycle::post_upgrade(None),
    }
    setup_timers();
}

#[query]
#[candid_method(query)]
async fn smart_contract_address() -> String {
    read_state(|s| s.ethereum_contract_address).to_string()
}

#[candid_method(update)]
#[update]
async fn get_canister_status() -> ic_cdk::api::management_canister::main::CanisterStatusResponse {
    ic_cdk::api::management_canister::main::canister_status(
        ic_cdk::api::management_canister::main::CanisterIdRecord {
            canister_id: ic_cdk::id(),
        },
    )
    .await
    .expect("failed to fetch canister status")
    .0
}

#[query]
#[candid_method(query)]
fn get_events(arg: GetEventsArg) -> GetEventsResult {
    const MAX_EVENTS_PER_RESPONSE: u64 = 100;

    fn map_event_source(
        EventSource {
            transaction_hash,
            log_index,
        }: EventSource,
    ) -> CandidEventSource {
        CandidEventSource {
            transaction_hash: transaction_hash.to_string(),
            log_index: log_index.into(),
        }
    }

    fn map_event(Event { timestamp, payload }: Event) -> CandidEvent {
        use ic_cketh_minter::endpoints::events::EventPayload as EP;
        CandidEvent {
            timestamp,
            payload: match payload {
                EventType::Init(args) => EP::Init(args),
                EventType::Upgrade(args) => EP::Upgrade(args),
                EventType::AcceptedMint(MintEvent {
                    transaction_hash,
                    block_number,
                    log_index,
                    from_address,
                    to_address,
                    token_id,
                }) => EP::AcceptedTransfer {
                    transaction_hash: transaction_hash.to_string(),
                    block_number: block_number.into(),
                    log_index: log_index.into(),
                    from_address: from_address.to_string(),
                    to_address: to_address.to_string(),
                    token_id: into_nat(token_id),
                },
                EventType::InvalidTransfer {
                    event_source,
                    reason,
                } => EP::InvalidTransfer {
                    event_source: map_event_source(event_source),
                    reason,
                },
                EventType::MintedNft { event_source } => EP::MintedNft {
                    event_source: map_event_source(event_source),
                },
                EventType::SyncedToBlock { block_number } => EP::SyncedToBlock {
                    block_number: block_number.into(),
                },
                EventType::SkippedBlock(block_number) => EP::SkippedBlock {
                    block_number: block_number.into(),
                },
            },
        }
    }

    let events = storage::with_event_iter(|it| {
        it.skip(arg.start as usize)
            .take(arg.length.min(MAX_EVENTS_PER_RESPONSE) as usize)
            .map(map_event)
            .collect()
    });

    GetEventsResult {
        events,
        total_event_count: storage::total_event_count(),
    }
}

#[query]
fn http_request(req: HttpRequest) -> HttpResponse {
    use ic_metrics_encoder::MetricsEncoder;

    // disable update calls for this method
    if ic_cdk::api::data_certificate().is_none() {
        ic_cdk::trap("update call rejected");
    }

    if req.path() == "/metrics" {
        let mut writer = MetricsEncoder::new(vec![], ic_cdk::api::time() as i64 / 1_000_000);

        fn encode_metrics(w: &mut MetricsEncoder<Vec<u8>>) -> std::io::Result<()> {
            const WASM_PAGE_SIZE_IN_BYTES: f64 = 65536.0;

            read_state(|s| {
                w.encode_gauge(
                    "cketh_minter_stable_memory_bytes",
                    ic_cdk::api::stable::stable_size() as f64 * WASM_PAGE_SIZE_IN_BYTES,
                    "Size of the stable memory allocated by this canister.",
                )?;

                w.gauge_vec("cycle_balance", "Cycle balance of this canister.")?
                    .value(
                        &[("canister", "cketh-minter")],
                        ic_cdk::api::canister_balance128() as f64,
                    )?;

                w.encode_gauge(
                    "cketh_minter_last_observed_block",
                    s.last_observed_block_number
                        .map(|n| n.as_f64())
                        .unwrap_or(0.0),
                    "The last Ethereum block the ckETH minter observed.",
                )?;

                w.encode_gauge(
                    "cketh_minter_last_processed_block",
                    s.last_scraped_block_number.as_f64(),
                    "The last Ethereum block the ckETH minter checked for deposits.",
                )?;

                w.encode_counter(
                    "cketh_minter_skipped_blocks",
                    s.skipped_blocks.len() as f64,
                    "Total count of Ethereum blocks that were skipped for deposits.",
                )?;

                w.gauge_vec(
                    "cketh_minter_accepted_deposits",
                    "The number of deposits the ckETH minter processed, by status.",
                )?
                .value(&[("status", "accepted")], s.minted_events.len() as f64)?
                .value(&[("status", "rejected")], s.invalid_events.len() as f64)?;

                w.encode_gauge(
                    "cketh_event_count",
                    storage::total_event_count() as f64,
                    "Total number of events in the event log.",
                )?;

                ic_cketh_minter::eth_rpc::encode_metrics(w)?;

                Ok(())
            })
        }

        match encode_metrics(&mut writer) {
            Ok(()) => HttpResponseBuilder::ok()
                .header("Content-Type", "text/plain; version=0.0.4")
                .with_body_and_content_length(writer.into_inner())
                .build(),
            Err(err) => {
                HttpResponseBuilder::server_error(format!("Failed to encode metrics: {}", err))
                    .build()
            }
        }
    } else if req.path() == "/dashboard" {
        use askama::Template;
        let dashboard = read_state(dashboard::DashboardTemplate::from_state);
        HttpResponseBuilder::ok()
            .header("Content-Type", "text/html; charset=utf-8")
            .with_body_and_content_length(dashboard.render().unwrap())
            .build()
    } else if req.path() == "/logs" {
        use ic_cketh_minter::logs::{Log, Priority, Sort};
        use std::str::FromStr;

        let max_skip_timestamp = match req.raw_query_param("time") {
            Some(arg) => match u64::from_str(arg) {
                Ok(value) => value,
                Err(_) => {
                    return HttpResponseBuilder::bad_request()
                        .with_body_and_content_length("failed to parse the 'time' parameter")
                        .build()
                }
            },
            None => 0,
        };

        let mut log: Log = Default::default();

        match req.raw_query_param("priority") {
            Some(priority_str) => match Priority::from_str(priority_str) {
                Ok(priority) => match priority {
                    Priority::Info => log.push_logs(Priority::Info),
                    Priority::TraceHttp => log.push_logs(Priority::TraceHttp),
                    Priority::Debug => log.push_logs(Priority::Debug),
                },
                Err(_) => log.push_all(),
            },
            None => log.push_all(),
        }

        log.entries
            .retain(|entry| entry.timestamp >= max_skip_timestamp);

        fn ordering_from_query_params(sort: Option<&str>, max_skip_timestamp: u64) -> Sort {
            match sort {
                Some(ord_str) => match Sort::from_str(ord_str) {
                    Ok(order) => order,
                    Err(_) => {
                        if max_skip_timestamp == 0 {
                            Sort::Ascending
                        } else {
                            Sort::Descending
                        }
                    }
                },
                None => {
                    if max_skip_timestamp == 0 {
                        Sort::Ascending
                    } else {
                        Sort::Descending
                    }
                }
            }
        }

        log.sort_logs(ordering_from_query_params(
            req.raw_query_param("sort"),
            max_skip_timestamp,
        ));

        const MAX_BODY_SIZE: usize = 3_000_000;
        HttpResponseBuilder::ok()
            .header("Content-Type", "application/json; charset=utf-8")
            .with_body_and_content_length(log.serialize_logs(MAX_BODY_SIZE))
            .build()
    } else {
        HttpResponseBuilder::not_found().build()
    }
}

#[cfg(feature = "debug_checks")]
#[query]
fn check_audit_log() {
    use ic_cketh_minter::state::audit::replay_events;

    emit_preupgrade_events();

    read_state(|s| {
        replay_events()
            .is_equivalent_to(s)
            .expect("replaying the audit log should produce an equivalent state")
    })
}

fn main() {}

/// Checks the real candid interface against the one declared in the did file
#[test]
fn check_candid_interface_compatibility() {
    fn source_to_str(source: &candid::utils::CandidSource) -> String {
        match source {
            candid::utils::CandidSource::File(f) => {
                std::fs::read_to_string(f).unwrap_or_else(|_| "".to_string())
            }
            candid::utils::CandidSource::Text(t) => t.to_string(),
        }
    }

    fn check_service_equal(
        new_name: &str,
        new: candid::utils::CandidSource,
        old_name: &str,
        old: candid::utils::CandidSource,
    ) {
        let new_str = source_to_str(&new);
        let old_str = source_to_str(&old);
        match candid::utils::service_equal(new, old) {
            Ok(_) => {}
            Err(e) => {
                eprintln!(
                    "{} is not compatible with {}!\n\n\
            {}:\n\
            {}\n\n\
            {}:\n\
            {}\n",
                    new_name, old_name, new_name, new_str, old_name, old_str
                );
                panic!("{:?}", e);
            }
        }
    }

    candid::export_service!();

    let new_interface = __export_service();

    // check the public interface against the actual one
    let old_interface = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("cketh_minter.did");

    check_service_equal(
        "actual ledger candid interface",
        candid::utils::CandidSource::Text(&new_interface),
        "declared candid interface in cketh_minter.did file",
        candid::utils::CandidSource::File(old_interface.as_path()),
    );
}
