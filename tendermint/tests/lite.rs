use serde::{de::Error as _, Deserialize, Deserializer, Serialize};
use serde_json;
use std;
use std::{fs, path::PathBuf};
use tendermint::validator::Set;
use tendermint::{block, lite, rpc, validator, Time};

#[derive(Deserialize, Clone, Debug)]
struct TestSuite {
    signed_header: rpc::endpoint::commit::SignedHeader,
    last_validators: Vec<validator::Info>,
    validators: Vec<validator::Info>,
}

#[derive(Deserialize, Clone, Debug)]
struct TestCases {
    test_cases: Vec<TestCase>,
}
#[derive(Deserialize, Clone, Debug)]
struct TestCase {
    name: String,
    description: String,
    initial: Initial,
    input: Vec<LiteBlock>,
    expected_output: Option<Vec<String>>,
}

#[derive(Clone, Debug)]
struct Duration(u64);

#[derive(Deserialize, Clone, Debug)]
struct Initial {
    signed_header: SignedHeader,
    next_validator_set: Set,
    trusting_period: Duration,
    now: Time,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct LiteBlock {
    signed_header: SignedHeader,
    validator_set: Set,
    next_validator_set: Set,
}

// TODO: this replicates the rpc::endpoint::commit::SignedHeader because the tests can contain an
// empty commit
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SignedHeader {
    /// Block header
    pub header: Option<block::Header>,
    /// Commit containing signatures for the header
    pub commit: Option<block::Commit>,
}

// Notes/Feedback:
// - the JSON contains for instance invalid signatures, this makes parsing the whole JSON
// fail in Rust:
//
// e.g. `Result::unwrap()` on an `Err` value: Error("signature error", line: 15474, column: 10)'
//
// - similarly some fields that are "null" in the header or the validator_set
//
// - also "wrong chain id"
// - same with an invalid proposer_address
//
// Suggestion: maybe put the invalid stuff that is invalid *not* from the lite client perspective
// but invalid data in separate file(s);
// in Rust we often already fail while decoding which makes decoding the
// test-file a bit useless (as it will err before we ran the tests that work)

fn read_json_fixture(name: &str) -> String {
    fs::read_to_string(PathBuf::from("./tests/support/lite/").join(name.to_owned() + ".json"))
        .unwrap()
}

#[test]
fn language_agnostic_test_cases() {
    let cases: TestCases = serde_json::from_str(&read_json_fixture("test_lite_client")).unwrap();
    println!("cases: {:?}", cases);
}

#[test]
fn check_verifier_with_mock_data() {
    let suite: TestSuite = serde_json::from_str(&read_json_fixture("basic")).unwrap();
    lite::verify_trusting(
        suite.signed_header.header.clone(),
        suite.signed_header,
        validator::Set::new(suite.last_validators),
        validator::Set::new(suite.validators),
    )
    .expect("verify_trusting failed");
}

impl<'de> Deserialize<'de> for Duration {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Duration(
            String::deserialize(deserializer)?
                .parse()
                .map_err(|e| D::Error::custom(format!("{}", e)))?,
        ))
    }
}

impl From<Duration> for std::time::Duration {
    fn from(d: Duration) -> std::time::Duration {
        std::time::Duration::from_nanos(d.0)
    }
}
