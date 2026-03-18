use super::ServerEngine;
use crate::{account_store::AccountStore, table::PokerTable};
use common::protocol::{ActionCmd, AllowedAction, Command, Event, HandStep, LobbyCmd, PokerVariant};
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

        let table = PokerTable::new();
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

    fn new_three_players_with_seed(seed: u64) -> Self {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("accounts.json");

        let mut store = AccountStore::load(&path).expect("load account store");
        store
            .create_account("alice", "1234", 1_000)
            .expect("create alice");
        store
            .create_account("bob", "1234", 1_000)
            .expect("create bob");
        store
            .create_account("carol", "1234", 1_000)
            .expect("create carol");

        let table = PokerTable::new();
        let mut engine = ServerEngine::new(store, table);

        for name in ["alice", "bob", "carol"] {
            let join_events = engine.apply(Command::Lobby(LobbyCmd::Join {
                player_name: name.to_string(),
                pin: "1234".to_string(),
            }));
            assert_nonempty_no_error(&join_events);
        }

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

fn assert_steps_exact(events: &[Event], expected: &[HandStep]) {
    let steps = step_transitions(events);
    assert_eq!(steps, expected, "unexpected step transition sequence");
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

fn last_action_requested_allowed(events: &[Event]) -> Vec<AllowedAction> {
    events
        .iter()
        .rev()
        .find_map(|e| match e {
            Event::ActionRequested { allowed, .. } => Some(allowed.clone()),
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

fn showdown_results(events: &[Event]) -> Vec<(String, String)> {
    events
        .iter()
        .find_map(|e| match e {
            Event::Showdown { results } => Some(
                results
                    .iter()
                    .map(|r| (r.player_name.clone(), r.hand_desc.clone()))
                    .collect(),
            ),
            _ => None,
        })
        .expect("expected Showdown event")
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

fn set_holdem_variant(engine: &mut ServerEngine) -> Vec<Event> {
    engine.apply(Command::Lobby(LobbyCmd::SetVariant {
        variant: PokerVariant::TexasHoldEm,
    }))
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

fn advance_holdem_to_flop_betting(
    engine: &mut ServerEngine,
) -> (Vec<Event>, Vec<Event>, Vec<Event>) {
    let start_events = start_to_betting_round1(engine);
    assert_nonempty_no_error(&start_events);
    assert_eq!(engine.table.current_step(), HandStep::BettingRound1);
    assert_eq!(engine.table.hand_state().board_cards().len(), 0);

    // Heads-up preflop: dealer/SB calls, BB checks.
    let first = last_action_requested_player(&start_events);
    let call_events = engine.apply(Command::Action(ActionCmd::Call { player_name: first }));
    assert_nonempty_no_error(&call_events);

    let second = last_action_requested_player(&call_events);
    let check_events = engine.apply(Command::Action(ActionCmd::Check {
        player_name: second,
    }));
    assert_nonempty_no_error(&check_events);
    assert_eq!(engine.table.current_step(), HandStep::BettingRound2);
    assert_eq!(engine.table.hand_state().board_cards().len(), 3);

    (start_events, call_events, check_events)
}

fn complete_current_holdem_betting_street_by_checks(
    engine: &mut ServerEngine,
    events_with_prompt: &[Event],
) -> (Vec<Event>, Vec<Event>) {
    let p1 = last_action_requested_player(events_with_prompt);
    let e1 = engine.apply(Command::Action(ActionCmd::Check { player_name: p1 }));
    assert_nonempty_no_error(&e1);

    let p2 = last_action_requested_player(&e1);
    let e2 = engine.apply(Command::Action(ActionCmd::Check { player_name: p2 }));
    assert_nonempty_no_error(&e2);

    (e1, e2)
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

#[test]
fn holdem_preflop_blinds_and_betting_order() {
    let mut fx = EngineFixture::new_three_players_with_seed(42);

    let set_variant = fx.engine.apply(Command::Lobby(LobbyCmd::SetVariant {
        variant: PokerVariant::TexasHoldEm,
    }));
    assert_nonempty_no_error(&set_variant);

    let start_events = start_to_betting_round1(&mut fx.engine);
    assert_nonempty_no_error(&start_events);
    assert_eq!(fx.engine.table.current_step(), HandStep::BettingRound1);

    let alice_id = fx.engine.accounts.get_by_name("alice").expect("alice").id;
    let bob_id = fx.engine.accounts.get_by_name("bob").expect("bob").id;
    let carol_id = fx.engine.accounts.get_by_name("carol").expect("carol").id;

    // Dealer starts at seat 0 (alice); action order left of dealer is bob, carol, alice.
    // Hold'em blinds: SB=bob, BB=carol, so UTG is alice.
    assert_eq!(last_action_requested_player(&start_events), "alice");

    let br = fx.engine.table.hand_state().betting().expect("betting round state");
    assert_eq!(fx.engine.table.hand_state().pot_size(), 15);
    assert_eq!(fx.engine.table.hand_state().current_bet(), 10);
    assert_eq!(br.contrib.get(&bob_id).copied(), Some(5)); // bob (SB)
    assert_eq!(br.contrib.get(&carol_id).copied(), Some(10)); // carol (BB)
    assert_eq!(br.contrib.get(&alice_id).copied(), Some(0)); // alice (UTG)

    let utg_call = fx.engine.apply(Command::Action(ActionCmd::Call {
        player_name: "alice".into(),
    }));
    assert_nonempty_no_error(&utg_call);
    assert_eq!(last_action_requested_player(&utg_call), "bob");

    let sb_call = fx.engine.apply(Command::Action(ActionCmd::Call {
        player_name: "bob".into(),
    }));
    assert_nonempty_no_error(&sb_call);
    assert_eq!(last_action_requested_player(&sb_call), "carol");
    assert!(
        last_action_requested_allowed(&sb_call).contains(&AllowedAction::Check),
        "big blind should be allowed to check when unraised"
    );

    let bb_check = fx.engine.apply(Command::Action(ActionCmd::Check {
        player_name: "carol".into(),
    }));
    assert_nonempty_no_error(&bb_check);
    assert_eq!(fx.engine.table.current_step(), HandStep::BettingRound2);
    assert_eq!(last_action_requested_player(&bb_check), "bob"); // left of dealer postflop
    assert_includes_steps(&bb_check, &[HandStep::DealCommunity, HandStep::BettingRound2]);
}

#[test]
fn holdem_showdown_emits_board_and_hole_cards() {
    let mut fx = EngineFixture::new_two_players_with_seed(42);

    let set_variant = fx.engine.apply(Command::Lobby(LobbyCmd::SetVariant {
        variant: PokerVariant::TexasHoldEm,
    }));
    assert_nonempty_no_error(&set_variant);

    let start_events = start_to_betting_round1(&mut fx.engine);
    assert_nonempty_no_error(&start_events);
    assert_eq!(fx.engine.table.current_step(), HandStep::BettingRound1);

    // Heads-up preflop: dealer/SB acts first, then BB may check.
    let pre1 = last_action_requested_player(&start_events);
    let pre1_events = fx.engine.apply(Command::Action(ActionCmd::Call { player_name: pre1 }));
    assert_nonempty_no_error(&pre1_events);
    let pre2 = last_action_requested_player(&pre1_events);
    let pre2_events = fx.engine.apply(Command::Action(ActionCmd::Check { player_name: pre2 }));
    assert_nonempty_no_error(&pre2_events);
    assert_eq!(fx.engine.table.current_step(), HandStep::BettingRound2);

    // Flop/turn/river betting rounds: both players check through.
    let mut terminal_events = Vec::new();
    for _ in 0..3 {
        let p1 = last_action_requested_player(if terminal_events.is_empty() {
            &pre2_events
        } else {
            &terminal_events
        });
        let e1 = fx.engine.apply(Command::Action(ActionCmd::Check { player_name: p1 }));
        assert_nonempty_no_error(&e1);

        let p2 = last_action_requested_player(&e1);
        let e2 = fx.engine.apply(Command::Action(ActionCmd::Check { player_name: p2 }));
        assert_nonempty_no_error(&e2);
        terminal_events = e2;
    }

    assert_eq!(fx.engine.table.current_step(), HandStep::WaitingForPlayers);
    assert_includes_steps(
        &terminal_events,
        &[HandStep::Showdown, HandStep::Payout, HandStep::WaitingForPlayers],
    );

    let showdown = showdown_results(&terminal_events);
    assert_eq!(showdown.len(), 2);
    for (_name, desc) in showdown {
        assert!(desc.contains("board="), "missing board in showdown desc: {desc}");
        assert!(desc.contains("hole="), "missing hole cards in showdown desc: {desc}");
        assert!(desc.contains("=>"), "missing best hand result in showdown desc: {desc}");
    }

    assert!(
        terminal_events
            .iter()
            .any(|e| matches!(e, Event::Payout { .. })),
        "expected payout event"
    );
}

#[test]
fn holdem_golden_flow_start_hand_posts_blinds_stops_at_preflop_betting() {
    let mut fx = EngineFixture::new_two_players_with_seed(42);
    let set_variant_events = set_holdem_variant(&mut fx.engine);
    assert_nonempty_no_error(&set_variant_events);

    let start_events = start_to_betting_round1(&mut fx.engine);

    assert_nonempty_no_error(&start_events);
    assert_eq!(fx.engine.table.current_step(), HandStep::BettingRound1);
    assert_non_negative_pot_and_contribs(&fx.engine);
    assert_steps_exact(
        &start_events,
        &[
            HandStep::CollectForcedBets,
            HandStep::DealPrivate,
            HandStep::BettingRound1,
        ],
    );
    assert_eq!(fx.engine.table.hand_state().board_cards().len(), 0);
}

#[test]
fn holdem_golden_flow_preflop_completes_flop_dealt_stops_at_flop_betting() {
    let mut fx = EngineFixture::new_two_players_with_seed(42);
    let set_variant_events = set_holdem_variant(&mut fx.engine);
    assert_nonempty_no_error(&set_variant_events);

    let (_start_events, _call_events, preflop_close_events) =
        advance_holdem_to_flop_betting(&mut fx.engine);

    assert_nonempty_no_error(&preflop_close_events);
    assert_eq!(fx.engine.table.current_step(), HandStep::BettingRound2);
    assert_non_negative_pot_and_contribs(&fx.engine);
    assert_steps_exact(
        &preflop_close_events,
        &[HandStep::DealCommunity, HandStep::BettingRound2],
    );
    assert_eq!(fx.engine.table.hand_state().board_cards().len(), 3);
}

#[test]
fn holdem_golden_flow_complete_streets_showdown_payout_waiting() {
    let mut fx = EngineFixture::new_two_players_with_seed(42);
    let set_variant_events = set_holdem_variant(&mut fx.engine);
    assert_nonempty_no_error(&set_variant_events);

    let (_start_events, _pre_call_events, flop_prompt_events) =
        advance_holdem_to_flop_betting(&mut fx.engine);
    assert_eq!(fx.engine.table.hand_state().board_cards().len(), 3);
    assert_non_negative_pot_and_contribs(&fx.engine);

    let (_flop_check1, flop_close_events) =
        complete_current_holdem_betting_street_by_checks(&mut fx.engine, &flop_prompt_events);
    assert_eq!(fx.engine.table.current_step(), HandStep::BettingRound2);
    assert_steps_exact(&flop_close_events, &[HandStep::DealCommunity, HandStep::BettingRound2]);
    assert_eq!(fx.engine.table.hand_state().board_cards().len(), 4);
    assert_non_negative_pot_and_contribs(&fx.engine);

    let (_turn_check1, turn_close_events) =
        complete_current_holdem_betting_street_by_checks(&mut fx.engine, &flop_close_events);
    assert_eq!(fx.engine.table.current_step(), HandStep::BettingRound2);
    assert_steps_exact(&turn_close_events, &[HandStep::DealCommunity, HandStep::BettingRound2]);
    assert_eq!(fx.engine.table.hand_state().board_cards().len(), 5);
    assert_non_negative_pot_and_contribs(&fx.engine);

    let (_river_check1, river_close_events) =
        complete_current_holdem_betting_street_by_checks(&mut fx.engine, &turn_close_events);
    assert_eq!(fx.engine.table.current_step(), HandStep::WaitingForPlayers);
    assert_steps_exact(
        &river_close_events,
        &[HandStep::Showdown, HandStep::Payout, HandStep::WaitingForPlayers],
    );
    assert_non_negative_pot_and_contribs(&fx.engine);
    // Board resets after payout when hand state is reset.
    assert_eq!(fx.engine.table.hand_state().board_cards().len(), 0);
}

fn 
