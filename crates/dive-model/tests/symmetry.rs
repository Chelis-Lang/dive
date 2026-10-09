//! Direct pinned-kernel controls, without enabling model reduction.
#![allow(clippy::unwrap_used, reason = "test assertions")]

use dive_model::symmetry::{Error, Group, Limits, Mode};

fn generators(text: &str) -> Vec<Vec<(usize, usize)>> {
    if text.is_empty() {
        return Vec::new();
    }
    text.split(';')
        .map(|function| {
            function
                .split(',')
                .filter(|p| !p.is_empty())
                .map(|pair| {
                    let (domain, range) = pair.split_once(':').unwrap();
                    (domain.parse().unwrap(), range.parse().unwrap())
                })
                .collect()
        })
        .collect()
}

#[test]
fn closure_matches_pinned_tlc_order_and_malformed_maps() {
    let inputs = include_str!("fixtures/symmetry-group/inputs.txt");
    let oracle = include_str!("fixtures/symmetry-group/tlc.jsonl");
    assert_eq!(inputs.lines().count(), 1329);
    assert_eq!(inputs.lines().count(), oracle.lines().count());
    for (input, expected) in inputs.lines().zip(oracle.lines()) {
        let fields: Vec<_> = input.split('|').collect();
        let universe: usize = fields[1].parse().unwrap();
        let functions = generators(fields[2]);
        let expected: serde_json::Value = serde_json::from_str(expected).unwrap();
        assert_eq!(expected["id"].as_str(), Some(fields[0]));
        let actual = Group::from_generators(universe, &functions, Mode::Tlc, Limits::default());
        if expected.get("error").is_some() {
            assert_eq!(
                actual.unwrap_err(),
                Error::TooFewGenerators,
                "{}",
                fields[0]
            );
            continue;
        }
        let actual = actual.unwrap();
        let rows: Vec<Vec<usize>> = actual.maps().iter().map(|p| p.images().to_vec()).collect();
        assert_eq!(
            serde_json::to_value(&rows).unwrap(),
            expected["maps"],
            "{}",
            fields[0]
        );
        let bijective = functions.iter().all(|function| {
            let mut images: Vec<_> = (0..universe).collect();
            for &(d, r) in function {
                images[d] = r;
            }
            images.sort_unstable();
            images == (0..universe).collect::<Vec<_>>()
        });
        let checked =
            Group::from_generators(universe, &functions, Mode::Checked, Limits::default());
        if bijective {
            assert_eq!(checked.unwrap().maps(), actual.maps(), "{}", fields[0]);
        } else {
            assert_eq!(checked.unwrap_err(), Error::NonBijective, "{}", fields[0]);
        }
    }
}

#[test]
fn disjoint_generators_close_to_the_product_and_keep_identity_implicit() {
    let group = Group::from_generators(
        4,
        &generators("0:1,1:0;2:3,3:2"),
        Mode::Checked,
        Limits::default(),
    )
    .unwrap();
    assert_eq!(
        group
            .maps()
            .iter()
            .map(dive_model::symmetry::Permutation::images)
            .collect::<Vec<_>>(),
        [&[1, 0, 2, 3][..], &[0, 1, 3, 2][..], &[1, 0, 3, 2][..],]
    );
    assert_eq!(group.maps()[2].image(3), Some(2));
    assert_eq!(group.maps()[2].image(4), None);
}

#[test]
fn partial_maps_are_checked_after_identity_extension() {
    let input = generators("0:1;0:0");
    assert_eq!(
        Group::from_generators(2, &input, Mode::Checked, Limits::default()).unwrap_err(),
        Error::NonBijective
    );
    assert_eq!(
        Group::from_generators(2, &input, Mode::Tlc, Limits::default())
            .unwrap()
            .maps()[0]
            .images(),
        [1, 1]
    );
    let identities =
        Group::from_generators(2, &generators("0:0;1:1"), Mode::Checked, Limits::default())
            .unwrap();
    assert_eq!(identities.maps(), []);
}

#[test]
fn invalid_indices_and_domains_are_errors_in_both_modes() {
    for mode in [Mode::Checked, Mode::Tlc] {
        for input in ["2:0;0:0", "0:2;0:0"] {
            assert_eq!(
                Group::from_generators(2, &generators(input), mode, Limits::default()).unwrap_err(),
                Error::UnknownModelValue
            );
        }
        assert_eq!(
            Group::from_generators(2, &generators("0:1,0:0;0:0"), mode, Limits::default())
                .unwrap_err(),
            Error::DuplicateDomain
        );
    }
}

#[test]
fn every_resource_refuses_without_exposing_a_partial_group() {
    let input = generators("0:1,1:0;2:3,3:2");
    for (limits, reason) in [
        (
            Limits {
                model_values: 3,
                ..Limits::default()
            },
            "model values",
        ),
        (
            Limits {
                maps: 2,
                ..Limits::default()
            },
            "maps",
        ),
        (
            Limits {
                cells: 15,
                ..Limits::default()
            },
            "cells",
        ),
        (
            Limits {
                work: 0,
                ..Limits::default()
            },
            "work",
        ),
    ] {
        assert_eq!(
            Group::from_generators(4, &input, Mode::Tlc, limits).unwrap_err(),
            Error::Limit(reason)
        );
    }
    assert_eq!(
        Group::from_generators(
            4,
            &input,
            Mode::Tlc,
            Limits {
                cells: 16,
                ..Limits::default()
            }
        )
        .unwrap()
        .maps()
        .len(),
        3
    );
    assert_eq!(
        Group::from_generators(
            usize::MAX,
            &input,
            Mode::Tlc,
            Limits {
                model_values: usize::MAX,
                cells: usize::MAX - 1,
                ..Limits::default()
            }
        )
        .unwrap_err(),
        Error::Limit("cells")
    );
    // Repeated empty functions still consume work even with no dense cells.
    assert_eq!(
        Group::from_generators(
            0,
            &vec![Vec::new(); 20],
            Mode::Tlc,
            Limits {
                work: 10,
                ..Limits::default()
            }
        )
        .unwrap_err(),
        Error::Limit("work")
    );
}
