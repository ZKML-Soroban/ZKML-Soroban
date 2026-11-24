use super::*;
use soroban_sdk::testutils::{Address as _, Events};
use soroban_sdk::IntoVal;

#[test]
fn verify_emits_verified_event_with_model_hash_and_output() {
    let env = Env::default();
    // This test calls the internal emitter directly through `as_contract`,
    // so the constructor arguments are placeholders; it never verifies.
    env.mock_all_auths();
    let contract_id = env.register(
        ZkmlVerifierContract,
        (
            Address::generate(&env),
            Bytes::from_slice(&env, &[0u8; 32]),
            None::<VerificationKey>,
        ),
    );

    let model_hash = Bytes::from_slice(&env, &[0xabu8; 32]);
    let output = Bytes::from_slice(&env, &[1, 2, 3, 4, 5, 6, 7, 8]);
    let record = InferenceRecord {
        model_hash: model_hash.clone(),
        output: output.clone(),
        class_label: 1,
        verified_at: 1234,
    };

    env.as_contract(&contract_id, || {
        ZkmlVerifierContract::emit_verified_event(&env, &record);
    });

    assert_eq!(
        env.events().all(),
        vec![
            &env,
            (
                contract_id.clone(),
                (symbol_short!("verified"), model_hash.clone()).into_val(&env),
                (1234u32, output.clone()).into_val(&env),
            ),
        ]
    );
}
