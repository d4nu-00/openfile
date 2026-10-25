//! The scheduler (an Anki-style SM-2 variant) and the study queue.
use crate::srs::deck::{Card, CardState, Deck, DeckOptions, Grade, Kind, NewOrder};
use serde::Serialize;
use sha2::{Digest, Sha256};

pub const MINUTE: i64 = 60_000;
pub const DAY: i64 = 86_400_000;
/// A "day" starts at 04:00 local time.
const DAY_START_HOUR: i64 = 4;
/// A learning card due within this many minutes is shown early when
/// nothing else is due.
pub const LEARN_AHEAD_MINUTES: i64 = 20;
/// The ease never drops below this.
const MIN_EASE: f64 = 1.3;
/// Mature cards have an interval of at least this many days.
const MATURE_DAYS: f64 = 21.0;
/// Review answers of this many days count for the retention.
const RETENTION_DAYS: i64 = 30;

fn minutes(m: f64) -> i64 {
    (m * MINUTE as f64).round() as i64
}

fn days(d: f64) -> i64 {
    (d * DAY as f64).round() as i64
}

/// A deterministic number in [0, 1) from a card and its repetitions.
fn unit_hash(text: &str) -> f64 {
    let digest = Sha256::digest(text.as_bytes());
    let bits = u64::from_be_bytes(digest[..8].try_into().expect("eight bytes"));
    bits as f64 / (u64::MAX as f64 + 1.0)
}

/// Intervals of 3 days or more vary by up to 5% either way, so cards
/// answered together do not all come back together.
fn fuzzed(interval: f64, card_id: &str, reps: u32) -> f64 {
    if interval < 3.0 {
        return interval;
    }
    interval * (1.0 + (unit_hash(&format!("{card_id}:{reps}")) * 2.0 - 1.0) * 0.05)
}

fn graduate(s: &mut CardState, interval: Option<f64>, now_ms: i64, opts: &DeckOptions) {
    if let Some(interval) = interval {
        s.interval = interval;
    }
    s.interval = s.interval.min(opts.max_interval);
    s.kind = Kind::Review;
    s.step = 0;
    s.due = now_ms + days(s.interval);
}

/// A new, learning or relearning card.
fn answer_in_steps(s: &mut CardState, grade: Grade, now_ms: i64, opts: &DeckOptions) {
    let relearning = s.kind == Kind::Relearning;
    if s.kind == Kind::New {
        s.kind = Kind::Learning;
        s.step = 0;
    }
    let steps = if relearning {
        &opts.relearning_steps
    } else {
        &opts.learning_steps
    };
    // After a lapse the interval stays as the lapse set it.
    let graduating = |good: bool| {
        (!relearning).then_some(if good {
            opts.graduating_interval
        } else {
            opts.easy_interval
        })
    };
    if grade == Grade::Easy {
        return graduate(s, graduating(false), now_ms, opts);
    }
    if steps.is_empty() {
        return graduate(s, graduating(true), now_ms, opts);
    }
    let last = steps.len() - 1;
    let step = (s.step as usize).min(last);
    match grade {
        Grade::Again => {
            s.step = 0;
            s.due = now_ms + minutes(steps[0]);
        }
        Grade::Hard => {
            s.step = step as u32;
            let wait = if step == last {
                steps[step] * 1.5
            } else {
                steps[step]
            };
            s.due = now_ms + minutes(wait);
        }
        _ if step < last => {
            s.step = step as u32 + 1;
            s.due = now_ms + minutes(steps[step + 1]);
        }
        _ => graduate(s, graduating(true), now_ms, opts),
    }
}

fn answer_review(s: &mut CardState, grade: Grade, now_ms: i64, opts: &DeckOptions, id: &str) {
    let interval = s.interval.max(1.0);
    let ease = s.ease.max(MIN_EASE);
    if grade == Grade::Again {
        s.lapses += 1;
        s.ease = (ease - 0.20).max(MIN_EASE);
        s.interval = (interval * 0.5).max(1.0).min(opts.max_interval);
        if let Some(&first) = opts.relearning_steps.first() {
            s.kind = Kind::Relearning;
            s.step = 0;
            s.due = now_ms + minutes(first);
        } else {
            s.due = now_ms + days(s.interval);
        }
        s.reps += 1;
        return;
    }
    let next = match grade {
        Grade::Hard => {
            s.ease = (ease - 0.15).max(MIN_EASE);
            (interval + 1.0).max(interval * 1.2)
        }
        Grade::Good => {
            s.ease = ease;
            (interval + 1.0).max(interval * ease)
        }
        _ => {
            s.ease = ease + 0.15;
            (interval + 1.0).max(interval * ease * 1.3)
        }
    };
    let next = fuzzed(next.min(opts.max_interval), id, s.reps).min(opts.max_interval);
    s.interval = next;
    s.due = now_ms + days(next);
    s.reps += 1;
}

/// The state of a card after it was answered at `now_ms`.
pub fn answer(
    state: &CardState,
    grade: Grade,
    now_ms: i64,
    opts: &DeckOptions,
    card_id: &str,
) -> CardState {
    let mut s = state.clone();
    match s.kind {
        Kind::Review => answer_review(&mut s, grade, now_ms, opts, card_id),
        _ => answer_in_steps(&mut s, grade, now_ms, opts),
    }
    s
}

/// The latest 04:00 local time at or before `now_ms`, in milliseconds since
/// the epoch; `utc_offset_min` is the local time's offset from UTC.
pub fn day_start(now_ms: i64, utc_offset_min: i32) -> i64 {
    let offset = i64::from(utc_offset_min) * MINUTE;
    let shift = DAY_START_HOUR * 60 * MINUTE;
    (now_ms + offset - shift).div_euclid(DAY) * DAY + shift - offset
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Queue {
    /// Card ids in the order they are studied.
    pub cards: Vec<String>,
    /// New cards the daily limit still allows.
    pub new_left: u32,
    /// Reviews the daily limit still allows.
    pub reviews_left: u32,
    /// Cards in `cards` that are learning or relearning.
    pub learning_due: u32,
    /// Cards in `cards` that are reviews.
    pub review_due: u32,
    /// Cards in `cards` that are new.
    pub new_due: u32,
    /// When the next learning card is due, when none is due now.
    pub next_learning_due: Option<i64>,
    /// The queue is learning cards shown ahead of their time.
    pub ahead: bool,
}

/// A card's state; a card that was never answered is new.
fn kind_of<'a>(deck: &'a Deck, card: &Card) -> (Kind, Option<&'a CardState>) {
    let state = deck.states.get(&card.id);
    (state.map_or(Kind::New, |s| s.kind), state)
}

/// The order of new cards that is random but stable: seeded from the deck.
fn random_key(deck_id: &str, card_id: &str) -> u64 {
    let digest = Sha256::digest(format!("{deck_id}:{card_id}").as_bytes());
    u64::from_be_bytes(digest[..8].try_into().expect("eight bytes"))
}

/// Reviews and new cards in one list: a new card after every
/// `max(1, reviews / new)` reviews.
fn interleave(reviews: Vec<String>, new: Vec<String>) -> Vec<String> {
    let every = if new.is_empty() {
        usize::MAX
    } else {
        (reviews.len() / new.len()).max(1)
    };
    let mut out = Vec::with_capacity(reviews.len() + new.len());
    let mut new = new.into_iter();
    for (i, review) in reviews.into_iter().enumerate() {
        out.push(review);
        if (i + 1) % every == 0
            && let Some(card) = new.next()
        {
            out.push(card);
        }
    }
    out.extend(new);
    out
}

/// What the day's log says was answered: new cards and reviews.
fn answered_today(deck: &Deck, start: i64) -> (u32, u32) {
    let (mut new, mut reviews) = (0, 0);
    for entry in deck.log.iter().filter(|e| e.time >= start) {
        match entry.kind {
            Kind::New => new += 1,
            Kind::Review => reviews += 1,
            _ => {}
        }
    }
    (new, reviews)
}

pub fn queue(deck: &Deck, now_ms: i64, utc_offset_min: i32) -> Queue {
    let start = day_start(now_ms, utc_offset_min);
    let end = start + DAY;
    let (new_answered, reviews_answered) = answered_today(deck, start);
    let new_left = deck.options.new_per_day.saturating_sub(new_answered);
    let reviews_left = deck
        .options
        .reviews_per_day
        .saturating_sub(reviews_answered);
    let mut learning: Vec<(i64, u32, &str)> = vec![];
    let mut ahead: Vec<(i64, u32, &str)> = vec![];
    let mut reviews: Vec<(i64, u32, &str)> = vec![];
    let mut new: Vec<&Card> = vec![];
    let mut next_learning_due: Option<i64> = None;
    for card in deck.cards.iter().filter(|c| !c.suspended) {
        let (kind, state) = kind_of(deck, card);
        let due = state.map_or(0, |s| s.due);
        match kind {
            Kind::New => new.push(card),
            Kind::Learning | Kind::Relearning if due <= now_ms => {
                learning.push((due, card.order, &card.id));
            }
            Kind::Learning | Kind::Relearning => {
                next_learning_due = Some(next_learning_due.map_or(due, |d| d.min(due)));
                if due <= now_ms + LEARN_AHEAD_MINUTES * MINUTE {
                    ahead.push((due, card.order, &card.id));
                }
            }
            Kind::Review if due < end => reviews.push((due, card.order, &card.id)),
            Kind::Review => {}
        }
    }
    learning.sort();
    reviews.sort();
    reviews.truncate(reviews_left as usize);
    match deck.options.new_order {
        NewOrder::Source => new.sort_by_key(|c| c.order),
        NewOrder::Random => new.sort_by_key(|c| (random_key(&deck.id, &c.id), c.order)),
    }
    new.truncate(new_left as usize);
    let ids = |list: &[(i64, u32, &str)]| -> Vec<String> {
        list.iter().map(|&(_, _, id)| id.to_string()).collect()
    };
    let mut got = Queue {
        new_left,
        reviews_left,
        learning_due: learning.len() as u32,
        review_due: reviews.len() as u32,
        new_due: new.len() as u32,
        next_learning_due,
        ..Queue::default()
    };
    got.cards = ids(&learning);
    got.cards.extend(interleave(
        ids(&reviews),
        new.iter().map(|c| c.id.clone()).collect(),
    ));
    if got.cards.is_empty() && !ahead.is_empty() {
        ahead.sort();
        got.cards = ids(&ahead);
        got.learning_due = ahead.len() as u32;
        got.ahead = true;
    }
    got
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeckStats {
    pub total: u32,
    pub new: u32,
    pub learning: u32,
    pub due_today: u32,
    pub suspended: u32,
    pub mature: u32,
    pub reviews_today: u32,
    /// The share of review answers of the last 30 days that were not Again.
    pub retention: Option<f64>,
    /// Review cards due today (with the overdue), and on each of the next 6 days.
    pub forecast: [u32; 7],
}

pub fn stats(deck: &Deck, now_ms: i64, utc_offset_min: i32) -> DeckStats {
    let start = day_start(now_ms, utc_offset_min);
    let end = start + DAY;
    let mut out = DeckStats {
        total: deck.cards.len() as u32,
        ..DeckStats::default()
    };
    for card in &deck.cards {
        if card.suspended {
            out.suspended += 1;
            continue;
        }
        let (kind, state) = kind_of(deck, card);
        match (kind, state) {
            (Kind::New, _) => out.new += 1,
            (Kind::Learning | Kind::Relearning, _) => out.learning += 1,
            (Kind::Review, Some(state)) => {
                if state.interval >= MATURE_DAYS {
                    out.mature += 1;
                }
                let day = if state.due < end {
                    0
                } else {
                    ((state.due - end) / DAY + 1) as usize
                };
                if day == 0 {
                    out.due_today += 1;
                }
                if let Some(slot) = out.forecast.get_mut(day) {
                    *slot += 1;
                }
            }
            (Kind::Review, None) => {}
        }
    }
    let since = now_ms - RETENTION_DAYS * DAY;
    let (mut answers, mut kept) = (0u32, 0u32);
    for entry in deck.log.iter().filter(|e| e.kind == Kind::Review) {
        if entry.time >= start {
            out.reviews_today += 1;
        }
        if entry.time >= since {
            answers += 1;
            kept += u32::from(entry.grade != Grade::Again);
        }
    }
    out.retention = (answers > 0).then(|| f64::from(kept) / f64::from(answers));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::srs::deck::{Card, Colour, Kind, NewOrder, ReviewLog, SourceKind};
    use std::collections::BTreeMap;

    /// 2023-11-14T00:00:00Z.
    const D0: i64 = 1_699_920_000_000;
    const HOUR: i64 = 60 * MINUTE;
    const NOW: i64 = D0 + 10 * HOUR;

    fn opts() -> DeckOptions {
        DeckOptions::default()
    }

    fn st(kind: Kind, step: u32, interval: f64, ease: f64) -> CardState {
        CardState {
            kind,
            step,
            due: 0,
            interval,
            ease,
            reps: 3,
            lapses: 0,
        }
    }

    fn go(state: &CardState, grade: Grade) -> CardState {
        answer(state, grade, NOW, &opts(), "card")
    }

    fn near(value: f64, centre: f64) {
        assert!(
            value >= centre * 0.95 - 1e-9 && value <= centre * 1.05 + 1e-9,
            "{value} is not within 5% of {centre}"
        );
    }

    #[test]
    fn a_new_card_becomes_learning_and_again_returns_it_after_the_first_step() {
        let s = go(&CardState::default(), Grade::Again);
        assert_eq!((s.kind, s.step, s.due), (Kind::Learning, 0, NOW + MINUTE));
    }

    #[test]
    fn good_moves_through_the_steps_and_graduates_after_the_last() {
        let s = go(&CardState::default(), Grade::Good);
        assert_eq!(
            (s.kind, s.step, s.due),
            (Kind::Learning, 1, NOW + 10 * MINUTE)
        );
        let g = go(&s, Grade::Good);
        assert_eq!(g.kind, Kind::Review);
        assert_eq!(g.interval, 1.0);
        assert_eq!(g.due, NOW + DAY);
        assert_eq!(g.ease, 2.5);
    }

    #[test]
    fn easy_graduates_with_the_easy_interval() {
        let s = go(&CardState::default(), Grade::Easy);
        assert_eq!(
            (s.kind, s.interval, s.due),
            (Kind::Review, 4.0, NOW + 4 * DAY)
        );
        let l = go(&st(Kind::Learning, 0, 0.0, 2.5), Grade::Easy);
        assert_eq!((l.kind, l.interval), (Kind::Review, 4.0));
    }

    #[test]
    fn again_in_learning_goes_back_to_the_first_step() {
        let s = go(&st(Kind::Learning, 1, 0.0, 2.5), Grade::Again);
        assert_eq!((s.kind, s.step, s.due), (Kind::Learning, 0, NOW + MINUTE));
    }

    #[test]
    fn hard_repeats_the_step_and_takes_one_and_a_half_times_the_last() {
        let first = go(&st(Kind::Learning, 0, 0.0, 2.5), Grade::Hard);
        assert_eq!((first.step, first.due), (0, NOW + MINUTE));
        let last = go(&st(Kind::Learning, 1, 0.0, 2.5), Grade::Hard);
        assert_eq!((last.step, last.due), (1, NOW + 15 * MINUTE));
        // Hard on a new card is the first step.
        let new = go(&CardState::default(), Grade::Hard);
        assert_eq!(
            (new.kind, new.step, new.due),
            (Kind::Learning, 0, NOW + MINUTE)
        );
        // An only step is the last one.
        let one = DeckOptions {
            learning_steps: vec![4.0],
            ..opts()
        };
        let s = answer(
            &st(Kind::Learning, 0, 0.0, 2.5),
            Grade::Hard,
            NOW,
            &one,
            "c",
        );
        assert_eq!(s.due, NOW + 6 * MINUTE);
    }

    #[test]
    fn relearning_graduates_with_the_interval_the_lapse_set() {
        let again = go(&st(Kind::Relearning, 0, 5.0, 2.3), Grade::Again);
        assert_eq!(
            (again.kind, again.step, again.due),
            (Kind::Relearning, 0, NOW + 10 * MINUTE)
        );
        let hard = go(&st(Kind::Relearning, 0, 5.0, 2.3), Grade::Hard);
        assert_eq!((hard.kind, hard.due), (Kind::Relearning, NOW + 15 * MINUTE));
        let good = go(&st(Kind::Relearning, 0, 5.0, 2.3), Grade::Good);
        assert_eq!(
            (good.kind, good.interval, good.due),
            (Kind::Review, 5.0, NOW + 5 * DAY)
        );
        let easy = go(&st(Kind::Relearning, 0, 5.0, 2.3), Grade::Easy);
        assert_eq!((easy.kind, easy.interval), (Kind::Review, 5.0));
        assert_eq!(good.ease, 2.3);
    }

    #[test]
    fn a_lapse_halves_the_interval_lowers_the_ease_and_relearns() {
        let s = go(&st(Kind::Review, 0, 10.0, 2.5), Grade::Again);
        assert_eq!(s.kind, Kind::Relearning);
        assert_eq!((s.step, s.lapses), (0, 1));
        assert_eq!(s.interval, 5.0);
        assert!((s.ease - 2.3).abs() < 1e-9);
        assert_eq!(s.due, NOW + 10 * MINUTE);
        // The interval is never below a day.
        assert_eq!(
            go(&st(Kind::Review, 0, 1.0, 2.5), Grade::Again).interval,
            1.0
        );
        // Without relearning steps the card stays a review.
        let none = DeckOptions {
            relearning_steps: vec![],
            ..opts()
        };
        let s = answer(
            &st(Kind::Review, 0, 10.0, 2.5),
            Grade::Again,
            NOW,
            &none,
            "c",
        );
        assert_eq!(
            (s.kind, s.interval, s.due, s.lapses),
            (Kind::Review, 5.0, NOW + 5 * DAY, 1)
        );
    }

    #[test]
    fn hard_good_and_easy_follow_the_interval_rules_with_the_plus_one_floor() {
        // Floors: 1 day -> at least 2 days (below 3 days there is no fuzz).
        let hard = go(&st(Kind::Review, 0, 1.0, 2.5), Grade::Hard);
        assert_eq!(hard.interval, 2.0);
        assert!((hard.ease - 2.35).abs() < 1e-9);
        let good = go(&st(Kind::Review, 0, 1.0, 1.3), Grade::Good);
        assert_eq!(good.interval, 2.0);
        assert_eq!(good.due, NOW + 2 * DAY);
        let good = go(&st(Kind::Review, 0, 1.0, 2.5), Grade::Good);
        assert_eq!(good.interval, 2.5);
        assert_eq!(good.ease, 2.5);
        // Multipliers: hard 1.2, good ease, easy ease * 1.3 with the old ease.
        near(
            go(&st(Kind::Review, 0, 10.0, 2.5), Grade::Hard).interval,
            12.0,
        );
        near(
            go(&st(Kind::Review, 0, 20.0, 2.5), Grade::Good).interval,
            50.0,
        );
        let easy = go(&st(Kind::Review, 0, 10.0, 2.5), Grade::Easy);
        near(easy.interval, 32.5);
        assert!((easy.ease - 2.65).abs() < 1e-9);
        assert_eq!(easy.reps, 4);
    }

    #[test]
    fn the_ease_never_drops_below_1_3() {
        assert_eq!(go(&st(Kind::Review, 0, 10.0, 1.3), Grade::Hard).ease, 1.3);
        assert_eq!(go(&st(Kind::Review, 0, 10.0, 1.4), Grade::Again).ease, 1.3);
    }

    #[test]
    fn intervals_are_clamped_to_the_maximum() {
        let s = go(&st(Kind::Review, 0, 30_000.0, 2.5), Grade::Good);
        assert!(s.interval <= 36_500.0 && s.interval >= 36_500.0 * 0.95);
        let small = DeckOptions {
            max_interval: 100.0,
            ..opts()
        };
        for id in ["a", "b", "c", "d", "e", "f"] {
            let s = answer(
                &st(Kind::Review, 0, 90.0, 2.5),
                Grade::Easy,
                NOW,
                &small,
                id,
            );
            assert!(s.interval <= 100.0, "{}", s.interval);
        }
    }

    #[test]
    fn fuzz_is_within_five_percent_and_deterministic_per_card_and_reps() {
        let base = st(Kind::Review, 0, 20.0, 2.5);
        let a = answer(&base, Grade::Good, NOW, &opts(), "card-a");
        assert_eq!(a, answer(&base, Grade::Good, NOW, &opts(), "card-a"));
        let values: std::collections::BTreeSet<u64> = (0..20)
            .map(|i| {
                let s = answer(&base, Grade::Good, NOW, &opts(), &format!("card-{i}"));
                near(s.interval, 50.0);
                s.interval.to_bits()
            })
            .collect();
        assert!(values.len() > 10, "equal cards spread out");
        // The repetition count is part of the fuzz.
        let reps: std::collections::BTreeSet<u64> = (0..20)
            .map(|reps| {
                let s = CardState {
                    reps,
                    ..base.clone()
                };
                answer(&s, Grade::Good, NOW, &opts(), "card-a")
                    .interval
                    .to_bits()
            })
            .collect();
        assert!(reps.len() > 10);
        // Below 3 days nothing is fuzzed.
        for id in ["a", "b", "c", "d"] {
            let s = answer(
                &st(Kind::Review, 0, 1.0, 2.5),
                Grade::Good,
                NOW,
                &opts(),
                id,
            );
            assert_eq!(s.interval, 2.5);
        }
    }

    #[test]
    fn answering_a_review_counts_a_repetition() {
        assert_eq!(go(&st(Kind::Review, 0, 10.0, 2.5), Grade::Good).reps, 4);
        assert_eq!(go(&st(Kind::Review, 0, 10.0, 2.5), Grade::Again).reps, 4);
    }

    #[test]
    fn a_day_starts_at_four_in_the_morning_local_time() {
        let four = D0 + 4 * HOUR;
        assert_eq!(day_start(four, 0), four);
        assert_eq!(day_start(four - 1, 0), four - DAY);
        assert_eq!(day_start(four + 5 * HOUR, 0), four);
        // UTC+2: local 04:00 is 02:00 UTC.
        let two = D0 + 2 * HOUR;
        assert_eq!(day_start(two, 120), two);
        assert_eq!(day_start(two - 1, 120), two - DAY);
        // UTC-5: local 04:00 is 09:00 UTC.
        let nine = D0 + 9 * HOUR;
        assert_eq!(day_start(nine, -300), nine);
        assert_eq!(day_start(nine - 1, -300), nine - DAY);
        // A half-hour offset (UTC+5:30): local 04:00 is 22:30 UTC the day before.
        let half = D0 - 90 * MINUTE;
        assert_eq!(day_start(half + HOUR, 330), half);
        assert_eq!(day_start(half - 1, 330), half - DAY);
    }

    fn card(i: u32) -> Card {
        Card {
            id: format!("c{i}"),
            start_fen: "8/8/8/8/8/8/8/8 w - - 0 1".into(),
            moves: vec![],
            trainee: Colour::White,
            white: String::new(),
            black: String::new(),
            event: String::new(),
            date: String::new(),
            source_label: String::new(),
            order: i,
            suspended: false,
            removed: false,
        }
    }

    fn deck(n: u32) -> Deck {
        Deck {
            version: 1,
            id: "deck-one".into(),
            name: "Deck".into(),
            created: 0,
            source: String::new(),
            source_kind: SourceKind::Pgn,
            options: opts(),
            cards: (0..n).map(card).collect(),
            states: BTreeMap::new(),
            log: vec![],
        }
    }

    fn set(deck: &mut Deck, id: &str, kind: Kind, due: i64) {
        deck.states.insert(
            id.into(),
            CardState {
                kind,
                due,
                interval: 5.0,
                ..CardState::default()
            },
        );
    }

    fn q(deck: &Deck) -> Vec<String> {
        queue(deck, NOW, 0).cards
    }

    fn log(id: &str, time: i64, grade: Grade, kind: Kind) -> ReviewLog {
        ReviewLog {
            card_id: id.into(),
            time,
            grade,
            kind,
            mistakes: 0,
            hints: 0,
            millis: 1000,
        }
    }

    #[test]
    fn learning_cards_come_first_by_due_then_reviews_by_due_then_new_in_source_order() {
        let mut d = deck(8);
        set(&mut d, "c5", Kind::Learning, NOW - 5 * MINUTE);
        set(&mut d, "c4", Kind::Relearning, NOW - 9 * MINUTE);
        set(&mut d, "c3", Kind::Review, NOW - DAY);
        set(&mut d, "c2", Kind::Review, NOW - 2 * DAY);
        // One review card is not due yet, one learning card is later.
        set(&mut d, "c1", Kind::Review, D0 + DAY + 4 * HOUR);
        set(&mut d, "c0", Kind::Learning, NOW + 2 * HOUR);
        let got = queue(&d, NOW, 0);
        assert_eq!(
            got.cards,
            ["c4", "c5", "c2", "c6", "c3", "c7"].map(String::from)
        );
        assert_eq!((got.learning_due, got.review_due, got.new_due), (2, 2, 2));
        assert_eq!(got.next_learning_due, Some(NOW + 2 * HOUR));
        assert!(!got.ahead);
    }

    #[test]
    fn reviews_due_today_end_at_the_next_day_boundary() {
        let boundary = D0 + DAY + 4 * HOUR;
        let mut d = deck(3);
        set(&mut d, "c0", Kind::Review, boundary - 1);
        set(&mut d, "c1", Kind::Review, boundary);
        set(&mut d, "c2", Kind::Review, boundary + HOUR);
        assert_eq!(q(&d), ["c0"]);
        // With the offset the boundary moves: UTC+2 puts it at 02:00 UTC.
        let mut d = deck(2);
        let local = D0 + DAY + 2 * HOUR;
        set(&mut d, "c0", Kind::Review, local - 1);
        set(&mut d, "c1", Kind::Review, local);
        assert_eq!(queue(&d, NOW, 120).cards, ["c0"]);
    }

    #[test]
    fn the_daily_limits_count_the_answers_of_this_day_only() {
        let start = day_start(NOW, 0);
        let mut d = deck(6);
        d.options.new_per_day = 3;
        d.options.reviews_per_day = 2;
        for i in 0..3 {
            set(&mut d, &format!("c{i}"), Kind::Review, NOW - DAY);
        }
        // Yesterday's answers do not count; today's do.
        d.log = vec![
            log("c0", start - 1, Grade::Good, Kind::Review),
            log("c9", start - 1, Grade::Good, Kind::New),
            log("c8", start, Grade::Good, Kind::New),
            log("c0", NOW - HOUR, Grade::Good, Kind::Review),
            // Learning steps use neither limit.
            log("c7", NOW - HOUR, Grade::Good, Kind::Learning),
        ];
        let got = queue(&d, NOW, 0);
        assert_eq!((got.new_left, got.reviews_left), (2, 1));
        assert_eq!(got.cards, ["c0", "c3", "c4"]);
    }

    #[test]
    fn reviews_and_new_cards_are_interleaved() {
        // 4 reviews, 2 new: a new card after every 2 reviews.
        let mut d = deck(6);
        d.options.new_per_day = 2;
        for i in 0..4 {
            set(&mut d, &format!("c{i}"), Kind::Review, NOW - DAY + i as i64);
        }
        assert_eq!(q(&d), ["c0", "c1", "c4", "c2", "c3", "c5"]);
        // Fewer reviews than new cards: one after each.
        let mut d = deck(5);
        set(&mut d, "c0", Kind::Review, NOW - DAY);
        set(&mut d, "c1", Kind::Review, NOW - DAY + 1);
        assert_eq!(q(&d), ["c0", "c2", "c1", "c3", "c4"]);
    }

    #[test]
    fn suspended_cards_are_not_studied() {
        let mut d = deck(3);
        d.cards[0].suspended = true;
        set(&mut d, "c1", Kind::Review, NOW - DAY);
        d.cards[1].suspended = true;
        assert_eq!(q(&d), ["c2"]);
    }

    #[test]
    fn a_learning_card_due_within_twenty_minutes_is_shown_early_when_nothing_is_due() {
        let mut d = deck(2);
        d.options.new_per_day = 0;
        set(&mut d, "c0", Kind::Learning, NOW + 15 * MINUTE);
        set(&mut d, "c1", Kind::Learning, NOW + 25 * MINUTE);
        let got = queue(&d, NOW, 0);
        assert_eq!(got.cards, ["c0"]);
        assert!(got.ahead);
        assert_eq!(got.learning_due, 1);
        // Further away, the session ends.
        let mut d = deck(1);
        d.options.new_per_day = 0;
        set(&mut d, "c0", Kind::Learning, NOW + 21 * MINUTE);
        let got = queue(&d, NOW, 0);
        assert!(got.cards.is_empty() && !got.ahead);
        assert_eq!(got.next_learning_due, Some(NOW + 21 * MINUTE));
        // Other cards due: no learn ahead.
        let mut d = deck(2);
        set(&mut d, "c0", Kind::Learning, NOW + 5 * MINUTE);
        assert_eq!(q(&d), ["c1"]);
    }

    #[test]
    fn new_cards_follow_the_source_order_or_a_stable_random_order() {
        let mut d = deck(30);
        d.cards.reverse();
        d.options.new_per_day = 30;
        let source = q(&d);
        let expected: Vec<String> = (0..30).map(|i| format!("c{i}")).collect();
        assert_eq!(source, expected);
        d.options.new_order = NewOrder::Random;
        let random = q(&d);
        assert_eq!(random, q(&d), "stable");
        assert_ne!(random, expected);
        let mut sorted = random.clone();
        sorted.sort_by_key(|id| id[1..].parse::<u32>().unwrap());
        assert_eq!(sorted, expected);
        d.id = "deck-two".into();
        assert_ne!(q(&d), random, "seeded from the deck id");
    }

    #[test]
    fn the_statistics_count_the_cards() {
        let mut d = deck(8);
        let tomorrow = D0 + DAY + 4 * HOUR;
        set(&mut d, "c0", Kind::Learning, NOW + HOUR);
        set(&mut d, "c1", Kind::Relearning, NOW - HOUR);
        set(&mut d, "c2", Kind::Review, NOW - 3 * DAY);
        set(&mut d, "c3", Kind::Review, tomorrow + HOUR);
        set(&mut d, "c4", Kind::Review, tomorrow + 3 * DAY);
        d.states.get_mut("c3").unwrap().interval = 21.0;
        d.states.get_mut("c4").unwrap().interval = 20.9;
        d.cards[7].suspended = true;
        d.log = vec![
            log("c2", NOW - HOUR, Grade::Good, Kind::Review),
            log("c3", NOW - 2 * HOUR, Grade::Again, Kind::Review),
            log("c3", NOW - 40 * DAY, Grade::Again, Kind::Review),
            log("c4", NOW - 3 * DAY, Grade::Hard, Kind::Review),
            log("c4", NOW - 3 * DAY, Grade::Good, Kind::Review),
            log("c5", NOW - 3 * DAY, Grade::Good, Kind::New),
            log("c6", NOW - 4 * DAY, Grade::Again, Kind::Learning),
        ];
        let s = stats(&d, NOW, 0);
        assert_eq!(s.total, 8);
        assert_eq!(s.new, 2, "c5 and c6 are new; c7 is suspended");
        assert_eq!(s.learning, 2);
        assert_eq!(s.due_today, 1);
        assert_eq!(s.suspended, 1);
        assert_eq!(s.mature, 1);
        assert_eq!(s.reviews_today, 2);
        // 4 review answers in 30 days, one of them Again.
        assert_eq!(s.retention, Some(0.75));
        // Today (with the overdue), tomorrow, ... c3 is due tomorrow, c4 in 4 days.
        assert_eq!(s.forecast, [1, 1, 0, 0, 1, 0, 0]);
    }

    #[test]
    fn retention_is_unknown_without_review_answers() {
        assert_eq!(stats(&deck(1), NOW, 0).retention, None);
    }
}
