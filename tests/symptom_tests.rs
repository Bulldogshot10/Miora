use miora::domain::symptom::SymptomCode;
use miora::engine::symptom_model::{
    SymptomObservation,
    probability_on_cycle_day,
};

#[test]
fn symptom_codes_have_stable_storage_keys() {
    assert_eq!(SymptomCode::Cramps.as_str(), "cramps");
    assert_eq!(SymptomCode::from_str("bloating"), Some(SymptomCode::Bloating));
    assert_eq!(SymptomCode::from_str("unknown"), None);
}

#[test]
fn symptom_probability_uses_only_matching_cycle_days() {
    let history = [
        SymptomObservation { cycle_day: 22, occurred: true },
        SymptomObservation { cycle_day: 22, occurred: false },
        SymptomObservation { cycle_day: 22, occurred: true },
        SymptomObservation { cycle_day: 23, occurred: true },
    ];

    let probability = probability_on_cycle_day(&history, 22).unwrap();
    assert!((probability - 0.6).abs() < f64::EPSILON);
}

#[test]
fn symptom_probability_is_unknown_without_observations() {
    assert!(probability_on_cycle_day(&[], 22).is_none());
}
