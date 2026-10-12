//! Contract tests for the deterministic mutator and the survey harness.
use class15::fuzz::{Report, Rng};

#[test]
fn zero_iterations_do_not_call_linker_or_change_corpus() {
    let corpus = vec![vec![b"object".to_vec()]];
    let before = corpus.clone();
    let calls = std::cell::Cell::new(0);
    let report = class15::fuzz::survey(&corpus, 0, 1, &|_| {
        calls.set(calls.get() + 1);
        true
    });
    assert_eq!(report, Report::default());
    assert_eq!(calls.get(), 0);
    assert_eq!(corpus, before);
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn generator_matches_xorshift64_star() {
    let mut rng = Rng::new(1);
    assert_eq!(
        [rng.next_u64(), rng.next_u64(), rng.next_u64()],
        [
            0x47e4_ce4b_896c_dd1d,
            0xabcf_a6a8_e079_651d,
            0xb9d1_0d8f_eb73_1f57
        ]
    );
    let mut zero = Rng::new(0);
    assert_eq!(
        [zero.next_u64(), zero.next_u64()],
        [0x0d83_b3e2_9a21_487a, 0x54c4_4c79_f1fe_9d67]
    );
    let mut a = Rng::new(99);
    let mut b = a.clone();
    let draws: Vec<_> = (0..1000).map(|_| a.below(7)).collect();
    assert!(draws.iter().all(|d| *d < 7));
    assert_eq!(
        draws,
        (0..1000).map(|_| b.next_u64() % 7).collect::<Vec<_>>()
    );
    assert_eq!(Rng::new(5).below(1), 0);
}

#[test]
fn mutations_follow_the_menu() {
    let input: Vec<u8> = (0..16).collect();
    let mut rng = Rng::new(7);
    let produced: Vec<_> = (0..8)
        .map(|_| hex(&class15::fuzz::mutate(&input, &mut rng)))
        .collect();
    assert_eq!(
        produced,
        [
            "0001020304050607000000000c0d0e0f",
            "0001020304050607ffffffffffffffff",
            "000102030505060708090a0b0c0d0e0f",
            "000102030c05060708090a0b0c0d0e0f",
            "000102030405060709090a0b0c0d0e0f",
            "000102030405000708090a0b0c0d0e0f",
            "000102030405",
            "000102031000000008090a0b0c0d0e0f",
        ]
    );
    assert!(class15::fuzz::mutate(&[], &mut Rng::new(3)).is_empty());
    // Every mutation of short inputs stays in bounds and never grows the input.
    let mut rng = Rng::new(11);
    for len in 1..12usize {
        let short: Vec<u8> = (0..len as u8).collect();
        for _ in 0..200 {
            assert!(class15::fuzz::mutate(&short, &mut rng).len() <= len);
        }
    }
    let mut a = Rng::new(42);
    let mut b = Rng::new(42);
    for _ in 0..100 {
        assert_eq!(
            class15::fuzz::mutate(&input, &mut a),
            class15::fuzz::mutate(&input, &mut b)
        );
    }
}

#[test]
fn survey_counts_outcomes_and_catches_panics() {
    let corpus = vec![
        vec![b"first input".to_vec(), b"second".to_vec()],
        vec![b"alone".to_vec()],
    ];
    let accept_long = |inputs: &[&[u8]]| inputs.iter().map(|i| i.len()).sum::<usize>() > 10;
    let report = class15::fuzz::survey(&corpus, 50, 3, &accept_long);
    assert_eq!(report.accepted + report.rejected, 50);
    assert!(report.panics.is_empty());
    assert_eq!(class15::fuzz::survey(&corpus, 50, 3, &accept_long), report);

    let fragile = |inputs: &[&[u8]]| {
        if inputs.len() == 1 && inputs[0] != b"alone" {
            panic!("bad {}", inputs[0].len());
        }
        true
    };
    let report = class15::fuzz::survey(&corpus, 40, 9, &fragile);
    assert_eq!(
        report.accepted + report.rejected + report.panics.len() as u64,
        40
    );
    assert!(!report.panics.is_empty());
    for case in &report.panics {
        assert_eq!((case.sample, case.input), (1, 0));
        assert_eq!(case.iteration % 2, 1);
        assert!(case.message.starts_with("bad "), "{}", case.message);
    }
    // Replaying a recorded panic reproduces it exactly.
    let first = &report.panics[0];
    let mut rng = Rng::new(9);
    let mut replayed = Vec::new();
    for iteration in 0..=first.iteration {
        let sample = &corpus[(iteration % 2) as usize];
        let input = rng.below(sample.len() as u64) as usize;
        replayed = class15::fuzz::mutate(&sample[input], &mut rng);
    }
    assert_eq!(first.message, format!("bad {}", replayed.len()));

    let silent = |_: &[&[u8]]| -> bool { std::panic::panic_any(7u32) };
    let report = class15::fuzz::survey(&corpus, 3, 1, &silent);
    assert_eq!(report.panics.len(), 3);
    assert!(report
        .panics
        .iter()
        .all(|p| p.message == "<non-string panic>"));
    let calls = std::cell::Cell::new(0);
    let counted = |_: &[&[u8]]| {
        calls.set(calls.get() + 1);
        true
    };
    assert_eq!(
        class15::fuzz::survey(&[], 10, 1, &counted),
        Report::default()
    );
    assert_eq!(
        class15::fuzz::survey(&corpus, 0, 1, &counted),
        Report::default()
    );
    assert_eq!(calls.get(), 0, "empty surveys must not invoke the backend");
}
