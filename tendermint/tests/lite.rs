use failure::_core::convert::TryFrom;
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
// e.g. `Result::unwrap()` on an `Err` value: Error("signature error", line: 15474, column: 10)'
//
// - similarly some fields that are "null" in the header or the validator_set
//
// - also test case: "wrong chain id"
// - same with an invalid proposer_address
//
// Suggestions:
// - maybe put the invalid stuff that is invalid *not* from the lite client perspective
// but invalid data in separate file(s);
// in Rust we often already fail while decoding which makes decoding the
// test-file a bit useless (as it will err before we ran the tests that work)
//
// - AppHash: []byte("app_hash") (hex 6170705F68617368) is not a valid sha256 hash
// (and in Rust will parse as: 6170705F68617368000000000000000000000000000000000000000000000000
// which produces a different header hash as the Golang version.

fn read_json_fixture(name: &str) -> String {
    fs::read_to_string(PathBuf::from("./tests/support/lite/").join(name.to_owned() + ".json"))
        .unwrap()
}

#[test]
fn language_agnostic_test_cases() {
    let cases: TestCases = serde_json::from_str(&read_json_fixture("test_lite_client")).unwrap();
    for (i, tc) in cases.test_cases.iter().enumerate() {
        match tc.name.as_ref() {
            "verify" => test_verify(i, tc),
            _ => println!("No such test found: {} ", tc.name),
        }
    }
}

fn test_verify(tci: usize, tc: &TestCase) {
    println!("Running tc number {}: {}", tci, tc.description);
    for (i, input) in tc.input.iter().enumerate() {
        if i == 0 {
            //            h1 *types.SignedHeader,
            //            h1NextVals *types.ValidatorSet,
            //            h2 *types.SignedHeader,
            //            h2Vals *types.ValidatorSet,
            //
            //            testCase.Initial.SignedHeader,
            //            &testCase.Initial.NextValidatorSet,
            //            input.SignedHeader,
            //            &input.ValidatorSet,
            match lite::verify_trusting(
                tc.initial.signed_header.header.as_ref().unwrap().clone(),
                rpc::endpoint::commit::SignedHeader::try_from(tc.initial.signed_header.clone())
                    .unwrap(),
                input.validator_set.clone(),
                tc.initial.next_validator_set.clone(),
            ) {
                Err(e) => {
                    //                println!("Res expected: {:?}", res.err().unwrap());
                    //                println!("Res expected: {:?}", tc.expected_output.as_ref());
                    //                println!("header: {:?}", tc.initial.signed_header.header.clone());
                    //                println!("commit: {:?}", sh.unwrap());
                    println!("err {:?}", e);
                    assert_eq!(tc.expected_output.is_none(), false);
                    println!(
                        "expected output: {:?}",
                        tc.expected_output.as_ref().unwrap().get(0).unwrap()
                    );
                }
                Ok(()) => match &tc.expected_output {
                    None => assert_eq!(tc.expected_output.is_none(), true),
                    Some(eo) => println!(
                        "({}, {}): No error verifying but expected output: {:?}",
                        tci,
                        i,
                        eo.get(0).unwrap()
                    ),
                },
            }
        } else {
            let sh = rpc::endpoint::commit::SignedHeader::try_from(input.signed_header.clone());
            if sh.is_err() {
                println!("Error parsing signedHeader: {:?}  ", sh.err().unwrap());
                continue;
            }

            let prev_input = tc.input.get(i - 1).unwrap();
            match lite::verify_trusting(
                sh.clone().unwrap().header,
                sh.clone().unwrap(),
                prev_input.next_validator_set.clone(),
                input.validator_set.clone(),
            ) {
                Err(e) => {
                    //                println!("Res expected: {:?}", res.err().unwrap());
                    //                println!("Res expected: {:?}", tc.expected_output.as_ref());
                    //                println!("header: {:?}", tc.initial.signed_header.header.clone());
                    //                println!("commit: {:?}", sh.unwrap());
                    println!("err {:?}", e);
                    assert_eq!(tc.expected_output.is_none(), false);
                }
                Ok(()) => match &tc.expected_output {
                    None => assert_eq!(tc.expected_output.is_none(), true),
                    Some(eo) => println!(
                        "({}, {}):No error verifying but expected output: {:?}",
                        tci,
                        i,
                        eo.get(i).unwrap()
                    ),
                },
            }
        }
    }
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

impl TryFrom<SignedHeader> for rpc::endpoint::commit::SignedHeader {
    type Error = &'static str;
    fn try_from(sh: SignedHeader) -> Result<Self, Self::Error> {
        match sh.header {
            Some(header) => match sh.commit {
                Some(commit) => Ok(rpc::endpoint::commit::SignedHeader { header, commit }),
                None => Err("Missing commit"),
            },
            None => Err("Missing header"),
        }
    }
}
