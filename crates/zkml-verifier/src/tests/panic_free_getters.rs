use super::*;
use soroban_sdk::testutils::Address as _;

#[test]
fn getters_return_none_when_empty_and_some_when_populated() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let model_hash = Bytes::from_slice(&env, &[42u8; 32]);
    let vk = test_utils::create_dummy_vk(&env, 5);

    let contract_id = env.register(
        ZkmlVerifierContract,
        (admin.clone(), model_hash.clone(), Some(vk.clone())),
    );
    let client = ZkmlVerifierContractClient::new(&env, &contract_id);

    // Before any verification: get_result returns None
    assert_eq!(client.get_result(), None);
    // get_model_hash returns Some(model_hash)
    assert_eq!(client.get_model_hash(), Some(model_hash));
    assert_eq!(client.version(), VERSION);

    // If storage is unpopulated, get_model_hash returns None without trapping
    env.as_contract(&contract_id, || {
        env.storage().instance().remove(&MODEL_HASH);
    });
    assert_eq!(client.get_model_hash(), None);
}
