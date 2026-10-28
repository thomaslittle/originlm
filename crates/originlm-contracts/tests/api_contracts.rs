use originlm_contracts::*;
use serde_json::json;
#[test]
fn public_schema_contains_method_input_and_output_maps() {
    let schema = schema_document();
    assert!(
        schema["$defs"]["MethodInputs"]["properties"]
            .get("sessions.create")
            .is_some()
    );
    assert!(
        schema["$defs"]["MethodOutputs"]["properties"]
            .get("models.list")
            .is_some()
    );
    assert!(
        schema["$defs"]["MethodInputs"]["properties"]
            .get("connections.probe")
            .is_some()
    );
}
#[test]
fn method_shapes_do_not_accept_caller_identity_or_arbitrary_executable() {
    let schema = schema_document();
    assert_eq!(
        schema["$defs"]["SessionCreateParams"]["additionalProperties"],
        json!(false)
    );
    assert!(
        schema["$defs"]["SessionCreateParams"]["properties"]
            .get("executable")
            .is_none()
    );
    assert!(
        schema["$defs"]["SessionCreateParams"]["properties"]
            .get("workspaceId")
            .is_some()
    );
}
