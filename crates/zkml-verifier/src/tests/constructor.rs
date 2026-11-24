use super::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::Env;
use test_utils::create_dummy_vk;

#[test]
fn test_initialize() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let model_hash = Bytes::from_slice(&env, &[1u8; 32]);
    let vk = create_dummy_vk(&env, 5);
    env.mock_all_auths();
    let contract_id = env.register(
        ZkmlVerifierContract,
        (admin.clone(), model_hash.clone(), Some(vk.clone())),
    );
    let client = ZkmlVerifierContractClient::new(&env, &contract_id);
    // The constructor ran at deploy, so the state it sets is already in place.
    assert_eq!(client.get_admin(), admin);
    assert_eq!(client.get_model_hash(), Some(model_hash));
    assert_eq!(client.get_result(), None);
    assert_eq!(client.get_verification_count(), 0);
    assert!(!client.is_paused());
}

// The double-initialize test is gone: initialization is now a constructor,
// which the protocol runs exactly once at deploy, so a second call cannot
// be expressed.
