use super::ServerEngine;
use crate::{account_store::AccountStore, table::FiveCardTable};
use common::protocol::{ActionCmd, Command, Event, HandStep, LobbyCmd};
use tempfile::TempDir;

struct EngineFixture {
    _tmp: TempDir,
    engine: ServerEngine,
}

impl EngineFixture {
    fn new_two_players_with_seed(seed: u64) -> Self {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("accounts.json");

        let mut store = AccountStore::load(&path).expect("load account store");
        store
            .create_account("alice", "1234", 1_000)
            .expect("create alice");
        store
            .create_account("bob", "1234", 1_000)
            .expect("create bob");

        let table = FiveCardTable::new();
        let mut engine = ServerEngine::new(store, table);

        let join_alice = engine.apply(Command::Lobby(LobbyCmd::Join {
            player_name: "alice".to_string(),
            pin: "1234".to_string(),
        }));
        assert_nonempty_no_error(&join_alice);

        let join_bob = engine.apply(Command::Lobby(LobbyCmd::Join {
            player_name: "bob".to_string(),
            pin: "1234".to_string(),
        }));
        assert_nonempty_no_error(&join_bob);

        let seed_events = engine.apply(Command::Lobby(LobbyCmd::SetRngSeed { seed }));
        assert_nonempty_no_error(&seed_events);

        Self { _tmp: tmp, engine }
    }
}

fn assert_nonempty_no_error(events: &[Event]) {
    assert!(!events.is_empty(), "expected non-empty events");
    for e in events {
        if let Event::Error { message } = e {
            panic!("unexpected error event: {message}");
        }
    }
}

fn assert_non_negative_pot_and_contribs(engine: &ServerEngine) {
    let pot = engine.table.hand_state().pot_size() as i64;
    assert!(pot >= 0, "pot should not be negative");

    if let Some(br) = engine.table.hand_state().betting() {
        for (pid, amt) in &br.contrib {
            assert!(
                (*amt as i64) >= 0,
                "contribution should not be negative for pid {pid}"
            );
        }
    }
}

fn step_transitions(events: &[Event]) -> Vec<HandStep> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::StepChanged { step } => Some(*step),
            _ => None,
        })
        .collect()
}

fn assert_includes_steps(events: &[Event], expected: &[HandStep]) {
    let steps = step_transitions(events);
    for step in expected {
        assert!(
            steps.contains(step),
            "missing step transition {:?}; got {:?}",
            step,
            steps
        );
    }
}

fn last_action_requested_player(events: &[Event]) -> String {
    events
        .iter()
        .rev()
        .find_map(|e| match e {
            Event::ActionRequested { player_name, .. } => Some(player_name.clone()),
            _ => None,
        })
        .expect("expected ActionRequested event")
}

fn last_draw_requested_player(events: &[Event]) -> String {
    events
        .iter()
        .rev()
        .find_map(|e| match e {
            Event::DrawRequested { player_name, .. } => Some(player_name.clone()),
            _ => None,
        })
        .expect("expected DrawRequested event")
}

fn apply_check(engine: &mut ServerEngine, player_name: String) -> Vec<Event> {
    engine.apply(Command::Action(ActionCmd::Check { player_name }))
}

fn apply_draw_pat(engine: &mut ServerEngine, player_name: String) -> Vec<Event> {
    engine.apply(Command::Action(ActionCmd::Draw {
        player_name,
        discard: vec![],
    }))
}

fn start_to_betting_round1(engine: &mut ServerEngine) -> Vec<Event> {
    engine.apply(Command::Lobby(LobbyCmd::StartHand))
}

fn advance_to_draw_phase(engine: &mut ServerEngine) -> (Vec<Event>, Vec<Event>, Vec<Event>) {
    let start_events = start_to_betting_round1(engine);
    assert_nonempty_no_error(&start_events);
    assert_eq!(engine.table.current_step(), HandStep::BettingRound1);

    let p1 = last_action_requested_player(&start_events);
    let check1 = apply_check(engine, p1);
    assert_nonempty_no_error(&check1);

    let p2 = last_action_requested_player(&check1);
    let check2 = apply_check(engine, p2);
    assert_nonempty_no_error(&check2);
    assert_eq!(engine.table.current_step(), HandStep::DrawPhase);

    (start_events, check1, check2)
}

#[test]
fn golden_flow_start_hand_stops_at_betting_round1() {
    let mut fx = EngineFixture::new_two_players_with_seed(42);

    let events = start_to_betting_round1(&mut fx.engine);

    assert_nonempty_no_error(&events);
    assert_eq!(fx.engine.table.current_step(), HandStep::BettingRound1);
    assert_non_negative_pot_and_contribs(&fx.engine);
    assert_includes_steps(
        &events,
        &[
            HandStep::CollectForcedBets,
            HandStep::DealPrivate,
            HandStep::BettingRound1,
        ],
    );
}

#[test]
fn golden_flow_betting_completes_stops_at_draw_phase() {
    let mut fx = EngineFixture::new_two_players_with_seed(42);

    let (_start_events, _check1_events, check2_events) = advance_to_draw_phase(&mut fx.engine);

    assert_nonempty_no_error(&check2_events);
    assert_eq!(fx.engine.table.current_step(), HandStep::DrawPhase);
    assert_non_negative_pot_and_contribs(&fx.engine);
    assert_includes_steps(&check2_events, &[HandStep::DrawPhase]);
}

#[test]
fn golden_flow_draw_to_betting2_then_showdown_payout_waiting() {
    let mut fx = EngineFixture::new_two_players_with_seed(42);

    let (_start_events, _check1_events, check2_events) = advance_to_draw_phase(&mut fx.engine);
    let draw_p1 = last_draw_requested_player(&check2_events);

    let draw1_events = apply_draw_pat(&mut fx.engine, draw_p1);
    assert_nonempty_no_error(&draw1_events);

    let draw_p2 = last_draw_requested_player(&draw1_events);
    let draw2_events = apply_draw_pat(&mut fx.engine, draw_p2);
    assert_nonempty_no_error(&draw2_events);
    assert_eq!(fx.engine.table.current_step(), HandStep::BettingRound2);
    assert_non_negative_pot_and_contribs(&fx.engine);
    assert_includes_steps(&draw2_events, &[HandStep::BettingRound2]);

    let b2_p1 = last_action_requested_player(&draw2_events);
    let b2_check1_events = apply_check(&mut fx.engine, b2_p1);
    assert_nonempty_no_error(&b2_check1_events);
    assert_non_negative_pot_and_contribs(&fx.engine);

    let b2_p2 = last_action_requested_player(&b2_check1_events);
    let b2_check2_events = apply_check(&mut fx.engine, b2_p2);
    assert_nonempty_no_error(&b2_check2_events);

    assert_eq!(fx.engine.table.current_step(), HandStep::WaitingForPlayers);
    assert_non_negative_pot_and_contribs(&fx.engine);
    assert_includes_steps(
        &b2_check2_events,
        &[HandStep::Showdown, HandStep::Payout, HandStep::WaitingForPlayers],
    );
}
