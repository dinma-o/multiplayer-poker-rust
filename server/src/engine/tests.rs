use super::GameEngine;
use crate::{account_store::AccountStore, table::PokerTable};
use common::constants::seven_card_stud;
use common::protocol::{
    ActionCmd, AllowedAction, AnimationFace, AnimationStreet, AnimationTarget, Command, DealEvent,
    Event, HandStep, LobbyCmd, PlayerId, PokerVariant,
};
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

struct EngineFixture {
    _tmp: TempDir,
    engine: GameEngine,
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
        let mut engine = GameEngine::new(Arc::new(Mutex::new(store)), table);

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
        let mut engine = GameEngine::new(Arc::new(Mutex::new(store)), table);

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

fn assert_non_negative_pot_and_contribs(engine: &GameEngine) {
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

fn deal_events(events: &[Event]) -> Vec<DealEvent> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Deal { deal } => Some(deal.clone()),
            _ => None,
        })
        .collect()
}

fn apply_check(engine: &mut GameEngine, player_name: String) -> Vec<Event> {
    engine.apply(Command::Action(ActionCmd::Check { player_name }))
}

fn apply_draw_pat(engine: &mut GameEngine, player_name: String) -> Vec<Event> {
    engine.apply(Command::Action(ActionCmd::Draw {
        player_name,
        discard: vec![],
    }))
}

fn start_to_betting_round1(engine: &mut GameEngine) -> Vec<Event> {
    engine.apply(Command::Lobby(LobbyCmd::StartHand))
}

fn set_holdem_variant(engine: &mut GameEngine) -> Vec<Event> {
    engine.apply(Command::Lobby(LobbyCmd::SetVariant {
        variant: PokerVariant::TexasHoldEm,
    }))
}

fn set_stud_variant(engine: &mut GameEngine) -> Vec<Event> {
    engine.apply(Command::Lobby(LobbyCmd::SetVariant {
        variant: PokerVariant::SevenCardStud,
    }))
}

fn last_action_requested_min_raise(events: &[Event]) -> Option<i64> {
    events.iter().rev().find_map(|e| match e {
        Event::ActionRequested { min_raise, .. } => *min_raise,
        _ => None,
    })
}

fn last_action_requested_max_amount(events: &[Event]) -> Option<i64> {
    events.iter().rev().find_map(|e| match e {
        Event::ActionRequested { max_amount, .. } => *max_amount,
        _ => None,
    })
}

fn apply_passive_betting_action(
    engine: &mut GameEngine,
    events_with_prompt: &[Event],
) -> Vec<Event> {
    let player_name = last_action_requested_player(events_with_prompt);
    let allowed = last_action_requested_allowed(events_with_prompt);

    if allowed.contains(&AllowedAction::Call) {
        engine.apply(Command::Action(ActionCmd::Call { player_name }))
    } else {
        engine.apply(Command::Action(ActionCmd::Check { player_name }))
    }
}

fn close_current_round_passively_two_players(
    engine: &mut GameEngine,
    events_with_prompt: &[Event],
) -> Vec<Event> {
    let e1 = apply_passive_betting_action(engine, events_with_prompt);
    assert_nonempty_no_error(&e1);

    let e2 = apply_passive_betting_action(engine, &e1);
    assert_nonempty_no_error(&e2);

    e2
}

fn stud_suit_order_for_test(suit: common::card_deck::Suit) -> u8 {
    match suit {
        common::card_deck::Suit::Clubs => 0,
        common::card_deck::Suit::Diamonds => 1,
        common::card_deck::Suit::Hearts => 2,
        common::card_deck::Suit::Spades => 3,
    }
}

fn stud_bring_in_player_for_test(engine: &GameEngine, active_in_order: &[PlayerId]) -> PlayerId {
    active_in_order
        .iter()
        .copied()
        .filter_map(|pid| {
            let up_card = engine
                .table
                .hand_state()
                .player_up_cards(&pid)?
                .first()
                .copied()?;
            Some((
                pid,
                up_card.rank() as u8,
                stud_suit_order_for_test(up_card.suit()),
            ))
        })
        .min_by(|a, b| (a.1, a.2).cmp(&(b.1, b.2)))
        .map(|(pid, _, _)| pid)
        .expect("bring-in player")
}

fn stud_visible_rank_for_test(cards: &[common::card_deck::Card]) -> (u8, [u8; 4]) {
    use std::collections::HashMap;

    let mut counts: HashMap<u8, u8> = HashMap::new();
    for card in cards {
        *counts.entry(card.rank() as u8).or_insert(0) += 1;
    }

    let mut groups: Vec<(u8, u8)> = counts.iter().map(|(&rank, &count)| (count, rank)).collect();
    groups.sort_unstable_by(|a, b| b.cmp(a));

    let singles_desc = |skip_ranks: &[u8]| -> Vec<u8> {
        let mut singles: Vec<u8> = cards
            .iter()
            .map(|c| c.rank() as u8)
            .filter(|rank| !skip_ranks.contains(rank))
            .collect();
        singles.sort_unstable_by(|a, b| b.cmp(a));
        singles
    };

    if let Some(&(4, quad_rank)) = groups.first() {
        return (4, [quad_rank, 0, 0, 0]);
    }

    if let Some(&(3, trip_rank)) = groups.first() {
        let kickers = singles_desc(&[trip_rank]);
        return (
            3,
            [
                trip_rank,
                *kickers.get(0).unwrap_or(&0),
                *kickers.get(1).unwrap_or(&0),
                0,
            ],
        );
    }

    if groups.len() >= 2 && groups[0].0 == 2 && groups[1].0 == 2 {
        let high_pair = groups[0].1.max(groups[1].1);
        let low_pair = groups[0].1.min(groups[1].1);
        let kicker = singles_desc(&[high_pair, low_pair])
            .into_iter()
            .next()
            .unwrap_or(0);
        return (2, [high_pair, low_pair, kicker, 0]);
    }

    if let Some(&(2, pair_rank)) = groups.first() {
        let kickers = singles_desc(&[pair_rank]);
        return (
            1,
            [
                pair_rank,
                *kickers.get(0).unwrap_or(&0),
                *kickers.get(1).unwrap_or(&0),
                *kickers.get(2).unwrap_or(&0),
            ],
        );
    }

    let mut ranks: Vec<u8> = cards.iter().map(|c| c.rank() as u8).collect();
    ranks.sort_unstable_by(|a, b| b.cmp(a));
    (
        0,
        [
            *ranks.get(0).unwrap_or(&0),
            *ranks.get(1).unwrap_or(&0),
            *ranks.get(2).unwrap_or(&0),
            *ranks.get(3).unwrap_or(&0),
        ],
    )
}

fn stud_visible_opener_for_test(engine: &GameEngine, active_in_order: &[PlayerId]) -> PlayerId {
    let mut best_pid = None;
    let mut best_rank = None;

    for pid in active_in_order {
        let up_cards = engine
            .table
            .hand_state()
            .player_up_cards(pid)
            .expect("visible up cards");
        let rank = stud_visible_rank_for_test(up_cards);

        match best_rank {
            None => {
                best_rank = Some(rank);
                best_pid = Some(*pid);
            }
            Some(current_best) if rank > current_best => {
                best_rank = Some(rank);
                best_pid = Some(*pid);
            }
            _ => {}
        }
    }

    best_pid.expect("visible opener")
}

fn advance_to_draw_phase(engine: &mut GameEngine) -> (Vec<Event>, Vec<Event>, Vec<Event>) {
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

fn advance_holdem_to_flop_betting(engine: &mut GameEngine) -> (Vec<Event>, Vec<Event>, Vec<Event>) {
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
    engine: &mut GameEngine,
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
fn five_card_draw_initial_deal_emits_round_robin_private_cues() {
    let mut fx = EngineFixture::new_two_players_with_seed(42);

    let events = start_to_betting_round1(&mut fx.engine);
    let deals = deal_events(&events);
    let order = fx.engine.table.action_order();

    assert_eq!(deals.len(), 10);
    for (index, deal) in deals.iter().enumerate() {
        assert_eq!(deal.street, AnimationStreet::FiveCardDrawInitial);
        assert_eq!(deal.target, AnimationTarget::Seat);
        assert_eq!(deal.face, AnimationFace::Down);
        assert_eq!(deal.player_id, Some(order[index % order.len()]));
        assert_eq!(deal.target_index, index / order.len());
    }
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
        &[
            HandStep::Showdown,
            HandStep::Payout,
            HandStep::WaitingForPlayers,
        ],
    );
}

#[test]
fn five_card_draw_replacement_emits_one_cue_per_new_card() {
    let mut fx = EngineFixture::new_two_players_with_seed(42);
    let (_start_events, _check1_events, draw_phase_events) = advance_to_draw_phase(&mut fx.engine);
    let draw_player = last_draw_requested_player(&draw_phase_events);

    let draw_events = fx.engine.apply(Command::Action(ActionCmd::Draw {
        player_name: draw_player,
        discard: vec![3, 1],
    }));
    let deals = deal_events(&draw_events);

    assert_eq!(deals.len(), 2);
    assert_eq!(
        deals.iter().map(|deal| deal.street).collect::<Vec<_>>(),
        vec![
            AnimationStreet::FiveCardDrawReplacement,
            AnimationStreet::FiveCardDrawReplacement,
        ]
    );
    assert_eq!(
        deals
            .iter()
            .map(|deal| deal.target_index)
            .collect::<Vec<_>>(),
        vec![1, 3]
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

    let accounts = fx.engine.accounts.lock().expect("account store lock");
    let alice_id = accounts.get_by_name("alice").expect("alice").id;
    let bob_id = accounts.get_by_name("bob").expect("bob").id;
    let carol_id = accounts.get_by_name("carol").expect("carol").id;
    drop(accounts);

    // Dealer starts at seat 0 (alice); action order left of dealer is bob, carol, alice.
    // Hold'em blinds: SB=bob, BB=carol, so UTG is alice.
    assert_eq!(last_action_requested_player(&start_events), "alice");

    let br = fx
        .engine
        .table
        .hand_state()
        .betting()
        .expect("betting round state");
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
    assert_includes_steps(
        &bb_check,
        &[HandStep::DealCommunity, HandStep::BettingRound2],
    );
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
    let pre1_events = fx
        .engine
        .apply(Command::Action(ActionCmd::Call { player_name: pre1 }));
    assert_nonempty_no_error(&pre1_events);
    let pre2 = last_action_requested_player(&pre1_events);
    let pre2_events = fx
        .engine
        .apply(Command::Action(ActionCmd::Check { player_name: pre2 }));
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
        let e1 = fx
            .engine
            .apply(Command::Action(ActionCmd::Check { player_name: p1 }));
        assert_nonempty_no_error(&e1);

        let p2 = last_action_requested_player(&e1);
        let e2 = fx
            .engine
            .apply(Command::Action(ActionCmd::Check { player_name: p2 }));
        assert_nonempty_no_error(&e2);
        terminal_events = e2;
    }

    assert_eq!(fx.engine.table.current_step(), HandStep::WaitingForPlayers);
    assert_includes_steps(
        &terminal_events,
        &[
            HandStep::Showdown,
            HandStep::Payout,
            HandStep::WaitingForPlayers,
        ],
    );

    let showdown = showdown_results(&terminal_events);
    assert_eq!(showdown.len(), 2);
    for (_name, desc) in showdown {
        assert!(
            desc.contains("board="),
            "missing board in showdown desc: {desc}"
        );
        assert!(
            desc.contains("hole="),
            "missing hole cards in showdown desc: {desc}"
        );
        assert!(
            desc.contains("=>"),
            "missing best hand result in showdown desc: {desc}"
        );
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
fn holdem_flop_emits_three_ordered_board_cues() {
    let mut fx = EngineFixture::new_two_players_with_seed(42);
    let set_variant_events = set_holdem_variant(&mut fx.engine);
    assert_nonempty_no_error(&set_variant_events);

    let (_start_events, _call_events, flop_events) = advance_holdem_to_flop_betting(&mut fx.engine);
    let deals = deal_events(&flop_events);

    assert_eq!(deals.len(), 3);
    for (index, deal) in deals.iter().enumerate() {
        assert_eq!(deal.street, AnimationStreet::HoldemFlop);
        assert_eq!(deal.target, AnimationTarget::Board);
        assert_eq!(deal.face, AnimationFace::Board);
        assert_eq!(deal.player_id, None);
        assert_eq!(deal.target_index, index);
    }
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
    assert_steps_exact(
        &flop_close_events,
        &[HandStep::DealCommunity, HandStep::BettingRound2],
    );
    assert_eq!(fx.engine.table.hand_state().board_cards().len(), 4);
    assert_non_negative_pot_and_contribs(&fx.engine);

    let (_turn_check1, turn_close_events) =
        complete_current_holdem_betting_street_by_checks(&mut fx.engine, &flop_close_events);
    assert_eq!(fx.engine.table.current_step(), HandStep::BettingRound2);
    assert_steps_exact(
        &turn_close_events,
        &[HandStep::DealCommunity, HandStep::BettingRound2],
    );
    assert_eq!(fx.engine.table.hand_state().board_cards().len(), 5);
    assert_non_negative_pot_and_contribs(&fx.engine);

    let (_river_check1, river_close_events) =
        complete_current_holdem_betting_street_by_checks(&mut fx.engine, &turn_close_events);
    assert_eq!(fx.engine.table.current_step(), HandStep::WaitingForPlayers);
    assert_steps_exact(
        &river_close_events,
        &[
            HandStep::Showdown,
            HandStep::Payout,
            HandStep::WaitingForPlayers,
        ],
    );
    assert_non_negative_pot_and_contribs(&fx.engine);
    // Board resets after payout when hand state is reset.
    assert_eq!(fx.engine.table.hand_state().board_cards().len(), 0);
}

#[test]
fn stud_start_hand_deals_third_street_and_seeds_bring_in() {
    let mut fx = EngineFixture::new_three_players_with_seed(42);
    let set_variant_events = set_stud_variant(&mut fx.engine);
    assert_nonempty_no_error(&set_variant_events);

    let start_events = start_to_betting_round1(&mut fx.engine);
    assert_nonempty_no_error(&start_events);
    assert_eq!(fx.engine.table.current_step(), HandStep::BettingRound1);
    assert_steps_exact(
        &start_events,
        &[
            HandStep::CollectForcedBets,
            HandStep::DealPrivate,
            HandStep::BettingRound1,
        ],
    );

    let active_in_order: Vec<PlayerId> = fx
        .engine
        .table
        .action_order()
        .into_iter()
        .filter(|pid| fx.engine.table.is_active_player(*pid))
        .collect();
    let bring_in_pid = stud_bring_in_player_for_test(&fx.engine, &active_in_order);
    let bring_in_name = fx
        .engine
        .accounts
        .lock()
        .expect("account store lock")
        .name_from_id(bring_in_pid)
        .expect("bring-in name");

    for pid in &active_in_order {
        assert_eq!(
            fx.engine
                .table
                .hand_state()
                .player_hole_cards(pid)
                .expect("third street cards")
                .len(),
            3
        );
        assert_eq!(
            fx.engine
                .table
                .hand_state()
                .player_up_cards(pid)
                .expect("third street up card")
                .len(),
            1
        );
    }

    let bring_in_idx = active_in_order
        .iter()
        .position(|pid| *pid == bring_in_pid)
        .expect("bring-in index");
    let expected_first_actor = active_in_order[(bring_in_idx + 1) % active_in_order.len()];
    let expected_first_actor_name = fx
        .engine
        .accounts
        .lock()
        .expect("account store lock")
        .name_from_id(expected_first_actor)
        .expect("first actor name");

    let br = fx
        .engine
        .table
        .hand_state()
        .betting()
        .expect("betting state");
    assert_eq!(
        fx.engine.table.hand_state().pot_size(),
        15 + seven_card_stud::BRING_IN_AMOUNT
    );
    assert_eq!(fx.engine.table.hand_state().current_bet(), 5);
    assert_eq!(
        br.contrib.get(&bring_in_pid).copied(),
        Some(5),
        "bring-in player should have forced contribution"
    );
    assert_eq!(
        last_action_requested_player(&start_events),
        expected_first_actor_name
    );
    assert!(
        !last_action_requested_allowed(&start_events).contains(&AllowedAction::Check),
        "the player after the bring-in should not be allowed to check"
    );
    assert_ne!(bring_in_name, expected_first_actor_name);
}

#[test]
fn stud_third_street_emits_two_down_passes_then_one_up_pass() {
    let mut fx = EngineFixture::new_three_players_with_seed(42);
    let set_variant_events = set_stud_variant(&mut fx.engine);
    assert_nonempty_no_error(&set_variant_events);

    let start_events = start_to_betting_round1(&mut fx.engine);
    let deals = deal_events(&start_events);
    let active_in_order: Vec<PlayerId> = fx
        .engine
        .table
        .action_order()
        .into_iter()
        .filter(|pid| fx.engine.table.is_active_player(*pid))
        .collect();

    assert_eq!(deals.len(), active_in_order.len() * 3);
    for (index, deal) in deals.iter().enumerate().take(active_in_order.len() * 2) {
        assert_eq!(deal.street, AnimationStreet::StudThirdDown);
        assert_eq!(deal.target, AnimationTarget::Seat);
        assert_eq!(deal.face, AnimationFace::Down);
        assert_eq!(
            deal.player_id,
            Some(active_in_order[index % active_in_order.len()])
        );
    }
    for (index, deal) in deals.iter().enumerate().skip(active_in_order.len() * 2) {
        assert_eq!(deal.street, AnimationStreet::StudThirdUp);
        assert_eq!(deal.target, AnimationTarget::Seat);
        assert_eq!(deal.face, AnimationFace::Up);
        assert_eq!(
            deal.player_id,
            Some(active_in_order[(index - active_in_order.len() * 2) % active_in_order.len()])
        );
    }
}

#[test]
fn stud_completion_moves_to_fourth_street_with_visible_best_opener() {
    let mut fx = EngineFixture::new_three_players_with_seed(42);
    let set_variant_events = set_stud_variant(&mut fx.engine);
    assert_nonempty_no_error(&set_variant_events);

    let start_events = start_to_betting_round1(&mut fx.engine);
    assert_nonempty_no_error(&start_events);

    let first_actor = last_action_requested_player(&start_events);
    let complete_events = fx.engine.apply(Command::Action(ActionCmd::Raise {
        player_name: first_actor,
        amount: 5,
    }));
    assert_nonempty_no_error(&complete_events);

    let second_actor = last_action_requested_player(&complete_events);
    let second_call_events = fx.engine.apply(Command::Action(ActionCmd::Call {
        player_name: second_actor,
    }));
    assert_nonempty_no_error(&second_call_events);

    let third_actor = last_action_requested_player(&second_call_events);
    let close_events = fx.engine.apply(Command::Action(ActionCmd::Call {
        player_name: third_actor,
    }));
    assert_nonempty_no_error(&close_events);

    assert_eq!(fx.engine.table.current_step(), HandStep::BettingRound2);
    assert_steps_exact(
        &close_events,
        &[HandStep::DealFaceUp, HandStep::BettingRound2],
    );
    assert_eq!(fx.engine.table.hand_state().current_bet(), 0);

    let active_in_order: Vec<PlayerId> = fx
        .engine
        .table
        .action_order()
        .into_iter()
        .filter(|pid| fx.engine.table.is_active_player(*pid))
        .collect();
    let opener_pid = stud_visible_opener_for_test(&fx.engine, &active_in_order);
    let opener_name = fx
        .engine
        .accounts
        .lock()
        .expect("account store lock")
        .name_from_id(opener_pid)
        .expect("opener name");

    for pid in &active_in_order {
        assert_eq!(
            fx.engine
                .table
                .hand_state()
                .player_hole_cards(pid)
                .expect("fourth street total cards")
                .len(),
            4
        );
        assert_eq!(
            fx.engine
                .table
                .hand_state()
                .player_up_cards(pid)
                .expect("fourth street up cards")
                .len(),
            2
        );
    }

    assert_eq!(fx.engine.table.hand_state().pot_size(), 45);
    assert_eq!(last_action_requested_player(&close_events), opener_name);
    let allowed = last_action_requested_allowed(&close_events);
    assert!(allowed.contains(&AllowedAction::Check));
    assert!(allowed.contains(&AllowedAction::Bet));
}

#[test]
fn five_card_draw_prompt_reports_server_max_amounts() {
    let mut fx = EngineFixture::new_two_players_with_seed(42);

    let start_events = start_to_betting_round1(&mut fx.engine);
    assert_nonempty_no_error(&start_events);
    assert_eq!(last_action_requested_max_amount(&start_events), Some(25));

    let opener = last_action_requested_player(&start_events);
    let raise_prompt_events = fx.engine.apply(Command::Action(ActionCmd::Bet {
        player_name: opener,
        amount: 5,
    }));
    assert_nonempty_no_error(&raise_prompt_events);

    let allowed = last_action_requested_allowed(&raise_prompt_events);
    assert!(allowed.contains(&AllowedAction::Raise));
    assert_eq!(
        last_action_requested_min_raise(&raise_prompt_events),
        Some(5)
    );
    assert_eq!(
        last_action_requested_max_amount(&raise_prompt_events),
        Some(35)
    );
}

#[test]
fn five_card_draw_prompt_omits_raise_when_player_cannot_afford_it() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("accounts.json");

    let mut store = AccountStore::load(&path).expect("load account store");
    store
        .create_account("alice", "1234", 10)
        .expect("create alice");
    store
        .create_account("bob", "1234", 1_000)
        .expect("create bob");

    let table = PokerTable::new();
    let mut engine = GameEngine::new(Arc::new(Mutex::new(store)), table);

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

    let seed_events = engine.apply(Command::Lobby(LobbyCmd::SetRngSeed { seed: 7 }));
    assert_nonempty_no_error(&seed_events);

    let start_events = start_to_betting_round1(&mut engine);
    assert_nonempty_no_error(&start_events);

    let first_actor = last_action_requested_player(&start_events);
    let prompt_events = if first_actor == "bob" {
        let bet_events = engine.apply(Command::Action(ActionCmd::Bet {
            player_name: "bob".to_string(),
            amount: 5,
        }));
        assert_nonempty_no_error(&bet_events);
        bet_events
    } else {
        let check_events = engine.apply(Command::Action(ActionCmd::Check {
            player_name: "alice".to_string(),
        }));
        assert_nonempty_no_error(&check_events);

        let bet_events = engine.apply(Command::Action(ActionCmd::Bet {
            player_name: "bob".to_string(),
            amount: 5,
        }));
        assert_nonempty_no_error(&bet_events);
        bet_events
    };

    assert_eq!(last_action_requested_player(&prompt_events), "alice");
    let allowed = last_action_requested_allowed(&prompt_events);
    assert!(allowed.contains(&AllowedAction::Call));
    assert!(!allowed.contains(&AllowedAction::Raise));
    assert_eq!(last_action_requested_max_amount(&prompt_events), None);
}

#[test]
fn stud_start_requires_enough_for_ante_and_possible_bring_in() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("accounts.json");

    let mut store = AccountStore::load(&path).expect("load account store");
    store
        .create_account("alice", "1234", seven_card_stud::ANTE_AMOUNT as i64)
        .expect("create alice");
    store
        .create_account("bob", "1234", 1_000)
        .expect("create bob");

    let table = PokerTable::new();
    let mut engine = GameEngine::new(Arc::new(Mutex::new(store)), table);

    let set_variant = set_stud_variant(&mut engine);
    assert_nonempty_no_error(&set_variant);

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

    let start_events = engine.apply(Command::Lobby(LobbyCmd::StartHand));
    assert!(
        start_events.iter().any(|event| matches!(
            event,
            Event::Error { message }
            if message.contains("required forced bets")
        )),
        "expected start-hand failure for insufficient stud bring-in coverage"
    );
}

#[test]
fn stud_big_bet_streets_and_seventh_street_private_card() {
    let mut fx = EngineFixture::new_two_players_with_seed(42);
    let set_variant_events = set_stud_variant(&mut fx.engine);
    assert_nonempty_no_error(&set_variant_events);

    let start_events = start_to_betting_round1(&mut fx.engine);
    assert_nonempty_no_error(&start_events);

    let fourth_prompt_events =
        close_current_round_passively_two_players(&mut fx.engine, &start_events);
    assert_eq!(fx.engine.table.current_step(), HandStep::BettingRound2);
    assert_eq!(
        last_action_requested_min_raise(&fourth_prompt_events),
        Some(10)
    );

    let fifth_prompt_events =
        close_current_round_passively_two_players(&mut fx.engine, &fourth_prompt_events);
    assert_eq!(fx.engine.table.current_step(), HandStep::BettingRound3);
    assert_steps_exact(
        &fifth_prompt_events,
        &[HandStep::DealFaceUp, HandStep::BettingRound3],
    );
    assert_eq!(
        last_action_requested_min_raise(&fifth_prompt_events),
        Some(20)
    );

    let sixth_prompt_events =
        close_current_round_passively_two_players(&mut fx.engine, &fifth_prompt_events);
    assert_eq!(fx.engine.table.current_step(), HandStep::BettingRound4);
    assert_steps_exact(
        &sixth_prompt_events,
        &[HandStep::DealFaceUp, HandStep::BettingRound4],
    );
    assert_eq!(
        last_action_requested_min_raise(&sixth_prompt_events),
        Some(20)
    );

    let seventh_prompt_events =
        close_current_round_passively_two_players(&mut fx.engine, &sixth_prompt_events);
    assert_eq!(fx.engine.table.current_step(), HandStep::BettingRound5);
    assert_steps_exact(
        &seventh_prompt_events,
        &[HandStep::DealPrivate, HandStep::BettingRound5],
    );
    assert_eq!(
        last_action_requested_min_raise(&seventh_prompt_events),
        Some(20)
    );

    for pid in fx.engine.active_players_in_hand() {
        assert_eq!(
            fx.engine
                .table
                .hand_state()
                .player_hole_cards(&pid)
                .expect("seventh street cards")
                .len(),
            7
        );
        assert_eq!(
            fx.engine
                .table
                .hand_state()
                .player_up_cards(&pid)
                .expect("stud up cards")
                .len(),
            4
        );
    }
}

#[test]
fn stud_showdown_and_payout_complete() {
    let mut fx = EngineFixture::new_two_players_with_seed(42);
    let set_variant_events = set_stud_variant(&mut fx.engine);
    assert_nonempty_no_error(&set_variant_events);

    let start_events = start_to_betting_round1(&mut fx.engine);
    assert_nonempty_no_error(&start_events);

    let fourth_prompt_events =
        close_current_round_passively_two_players(&mut fx.engine, &start_events);
    let fifth_prompt_events =
        close_current_round_passively_two_players(&mut fx.engine, &fourth_prompt_events);
    let sixth_prompt_events =
        close_current_round_passively_two_players(&mut fx.engine, &fifth_prompt_events);
    let seventh_prompt_events =
        close_current_round_passively_two_players(&mut fx.engine, &sixth_prompt_events);
    let terminal_events =
        close_current_round_passively_two_players(&mut fx.engine, &seventh_prompt_events);

    assert_eq!(fx.engine.table.current_step(), HandStep::WaitingForPlayers);
    assert_steps_exact(
        &terminal_events,
        &[
            HandStep::Showdown,
            HandStep::Payout,
            HandStep::WaitingForPlayers,
        ],
    );

    let showdown = showdown_results(&terminal_events);
    assert_eq!(showdown.len(), 2);
    for (_name, desc) in showdown {
        assert!(
            desc.contains("cards="),
            "missing full stud cards in showdown desc: {desc}"
        );
        assert!(
            desc.contains("up="),
            "missing stud up cards in showdown desc: {desc}"
        );
        assert!(
            desc.contains("=>"),
            "missing best-hand marker in showdown desc: {desc}"
        );
    }
}
