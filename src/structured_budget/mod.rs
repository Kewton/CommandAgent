//! One operator-issued wallet for every provider send of a run (Issue #976, G5).
//!
//! Three rules hold everything here up:
//!
//! - Before an HTTP POST, a reservation and a start for the same bytes are on disk in the wallet.
//! - A wallet is named only by the ID the operator issued. A session, a fresh start, a resume, a
//!   profile, a workspace, or a state directory never makes one, and no process-wide table holds
//!   one: the handle is passed by value.
//! - A wallet with an unresolved send or a set latch allows no send and no success.
//!
//! This stage is the money, the state machine, the ledger, and the clock. Nothing calls it yet: no
//! HTTP, no wiring, and with no wallet selected nothing here runs at all.

#![allow(
    dead_code,
    reason = "nothing calls this until the transport and the gates are wired in; removed then"
)]

pub(crate) mod clock;
pub(crate) mod identity;
pub(crate) mod ledger;
pub(crate) mod policy;
pub(crate) mod reservation;
pub(crate) mod settlement;

#[cfg(test)]
pub(crate) mod test_support {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};

    use serde_json::{Value, json};

    use super::clock::{BudgetClock, RunClock};
    use super::identity::{LogicalKind, SendPurpose};
    use super::policy::{RunBudgetPolicy, WireRoute};
    use super::reservation::ReserveRequest;

    pub(crate) const WALLET: &str = "wallet-alpha";

    /// A clock whose two readings a test moves apart.
    #[derive(Debug)]
    pub(crate) struct FakeClock {
        wall: AtomicU64,
        monotonic: AtomicU64,
    }

    impl FakeClock {
        pub(crate) fn at(wall: u64) -> Arc<Self> {
            Arc::new(Self {
                wall: AtomicU64::new(wall),
                monotonic: AtomicU64::new(0),
            })
        }

        pub(crate) fn advance(&self, by: u64) {
            self.wall.fetch_add(by, Ordering::SeqCst);
            self.monotonic.fetch_add(by, Ordering::SeqCst);
        }

        pub(crate) fn advance_monotonic_only(&self, by: u64) {
            self.monotonic.fetch_add(by, Ordering::SeqCst);
        }

        pub(crate) fn set_wall(&self, wall: u64) {
            self.wall.store(wall, Ordering::SeqCst);
        }

        /// What a new process sees: its monotonic reading starts over.
        pub(crate) fn new_process(&self) {
            self.monotonic.store(0, Ordering::SeqCst);
        }
    }

    impl BudgetClock for FakeClock {
        fn wall_ms(&self) -> u64 {
            self.wall.load(Ordering::SeqCst)
        }

        fn monotonic_ms(&self) -> u64 {
            self.monotonic.load(Ordering::SeqCst)
        }
    }

    pub(crate) fn run_clock(clock: &Arc<FakeClock>) -> RunClock {
        RunClock::new(clock.clone())
    }

    /// Everything wide, so that a test narrows the one limit it measures.
    pub(crate) fn policy_value() -> Value {
        json!({
            "protocol": "run-budget.policy/v1",
            "budget_id": WALLET,
            "limits": {
                "cost_usd": "1000",
                "physical_sends": 1000,
                "logical_global": 1000,
                "logical_per_stage": 1000,
                "tokens": 1_000_000,
                "wall_seconds": 100_000,
                "final_repair_attempts": 1000,
                "final_repair_seconds": 100_000
            },
            "tariff": {
                "revision": "tariff-one",
                "input_per_million": "2",
                "cached_input_per_million": "1",
                "cache_write_per_million": "2",
                "output_per_million": "10",
                "request_fee": "0"
            },
            "route": {
                "api": "chat_completions",
                "model": "m-1",
                "upstream_only": ["u"],
                "allow_fallbacks": false,
                "effort": "low",
                "reasoning_in_output": true
            },
            "max_output_tokens": 50,
            "max_input_tokens": 1000,
            "input_overhead": 4,
            "ledger_dir": "/operator/ledgers"
        })
    }

    pub(crate) fn decode(value: &Value) -> RunBudgetPolicy {
        RunBudgetPolicy::decode(value.to_string().as_bytes(), WALLET).unwrap()
    }

    pub(crate) fn policy() -> RunBudgetPolicy {
        decode(&policy_value())
    }

    pub(crate) fn policy_with(change: impl FnOnce(&mut Value)) -> RunBudgetPolicy {
        let mut value = policy_value();
        change(&mut value);
        decode(&value)
    }

    pub(crate) fn route() -> WireRoute {
        WireRoute {
            api: "chat_completions".to_string(),
            model: "m-1".to_string(),
            upstream_only: vec!["u".to_string()],
            allow_fallbacks: false,
            effort: "low".to_string(),
        }
    }

    /// 106 bytes and the 4 of overhead at 2 per million, 50 of output at 10 per million: 720 per
    /// million dollars, and 160 tokens. The shortest body a route can have is 102 bytes, and this
    /// one holds no `messages`, which a reservation does not ask for.
    pub(crate) const BODY: &[u8; 106] = br#"{"model":"m-1","max_tokens":50,"reasoning_effort":"low","provider":{"only":["u"],"allow_fallbacks":false}}"#;

    /// [`BODY`] asking for another output. Two digits keep its length.
    pub(crate) fn body_asking_for(max_tokens: u64) -> Vec<u8> {
        String::from_utf8(BODY.to_vec())
            .unwrap()
            .replace("\"max_tokens\":50", &format!("\"max_tokens\":{max_tokens}"))
            .into_bytes()
    }

    pub(crate) fn request() -> ReserveRequest<'static> {
        ReserveRequest {
            body: BODY,
            purpose: SendPurpose::Executor,
            final_repair: None,
        }
    }

    pub(crate) fn stage_kind(name: &str) -> LogicalKind {
        LogicalKind::Stage {
            stage: name.to_string(),
        }
    }

    /// A seeded step for a walk, with no new dependency.
    pub(crate) struct Rng(u64);

    impl Rng {
        pub(crate) fn seeded(seed: u64) -> Self {
            Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
        }

        pub(crate) fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
    }
}
