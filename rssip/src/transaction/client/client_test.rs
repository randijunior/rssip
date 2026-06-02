use super::*;
use crate::assert_eq_state;
use crate::error::Error;
use crate::test_utils::transaction::{
    CODE_100_TRYING, CODE_180_RINGING, CODE_202_ACCEPTED, CODE_301_MOVED_PERMANENTLY,
    CODE_404_NOT_FOUND, CODE_504_SERVER_TIMEOUT, CODE_603_DECLINE, ClientTestContext,
    SendRequestContext,
};

// INVITE Client tests

#[tokio::test]
async fn invite_transitions_to_calling_when_request_is_sent() {
    let ctx = SendRequestContext::setup(SipMethod::Invite).await;

    let uac = ClientTransaction::send_request_with_target(
        ctx.request,
        ctx.endpoint,
        (ctx.transport, ctx.destination),
    )
    .await
    .expect("error sending request");

    assert_eq!(
        uac.state(),
        State::Calling,
        "client INVITE must transition to the Calling state when sending the request"
    );
}

#[tokio::test(start_paused = true)]
async fn invite_should_not_start_timer_a_when_transport_is_reliable() {
    let mut ctx = ClientTestContext::setup_reliable(SipMethod::Invite).await;
    let expected_requests = 1;
    let expected_retrans = 0;

    let opt_err = ctx.client.receive_provisional_response().await.err();

    std::assert_matches!(
        opt_err,
        Some(Error::Transaction(TransactionError::Timeout)),
        "Expected Transaction::Timeout, got {opt_err:?}"
    );

    assert_eq!(
        ctx.transport.sent_count(),
        expected_requests + expected_retrans,
        "sent count should match {expected_requests} requests and {expected_retrans} retransmissions"
    );
}

#[tokio::test]
async fn invite_transitions_from_calling_to_proceeding_when_receiving_1xx_response() {
    let mut ctx = ClientTestContext::setup(SipMethod::Invite).await;

    ctx.server.respond(CODE_100_TRYING).await;

    ctx.client
        .receive_provisional_response()
        .await
        .expect("Error receiving provisional response")
        .expect("Expected provisional response, but received None");

    assert_eq_state!(
        ctx.state,
        State::Proceeding,
        "client INVITE must transition to the Proceeding state when receiving 1xx response"
    );
}

#[tokio::test]
async fn invite_transitions_from_calling_to_completed_when_receiving_3xx_response() {
    let mut ctx = ClientTestContext::setup(SipMethod::Invite).await;

    ctx.server.respond(CODE_301_MOVED_PERMANENTLY).await;

    ctx.client
        .receive_final_response()
        .await
        .expect("Error receiving final response");

    assert_eq_state!(
        ctx.state,
        State::Completed,
        "client INVITE must transition to the Completed state when receiving 3xx response"
    );
}

#[tokio::test]
async fn invite_transitions_from_calling_to_completed_when_receiving_4xx_response() {
    let mut ctx = ClientTestContext::setup(SipMethod::Invite).await;

    ctx.server.respond(CODE_404_NOT_FOUND).await;

    ctx.client
        .receive_final_response()
        .await
        .expect("Error receiving final response");

    assert_eq_state!(
        ctx.state,
        State::Completed,
        "client INVITE must transition to the Completed state when receiving 4xx response"
    );
}

#[tokio::test]
async fn invite_transitions_from_calling_to_completed_when_receiving_5xx_response() {
    let mut ctx = ClientTestContext::setup(SipMethod::Invite).await;

    ctx.server.respond(CODE_504_SERVER_TIMEOUT).await;

    ctx.client
        .receive_final_response()
        .await
        .expect("Error receiving final response");

    assert_eq_state!(
        ctx.state,
        State::Completed,
        "client INVITE must transition to the Completed state when receiving 5xx response"
    );
}

#[tokio::test]
async fn invite_transitions_from_calling_to_completed_when_receiving_6xx_response() {
    let mut ctx = ClientTestContext::setup(SipMethod::Invite).await;

    ctx.server.respond(CODE_603_DECLINE).await;

    ctx.client
        .receive_final_response()
        .await
        .expect("Error receiving final response");

    assert_eq_state!(
        ctx.state,
        State::Completed,
        "client INVITE must transition to the Completed state when receiving 6xx response"
    );
}

#[tokio::test]
async fn invite_transitions_from_calling_to_terminated_when_receiving_2xx_response() {
    let mut ctx = ClientTestContext::setup(SipMethod::Invite).await;

    ctx.server.respond(CODE_202_ACCEPTED).await;

    ctx.client
        .receive_final_response()
        .await
        .expect("Error receiving final response");

    assert_eq_state!(
        ctx.state,
        State::Terminated,
        "client INVITE must transition to the Terminated state when receiving 2xx response"
    );
}

#[tokio::test(start_paused = true)]
async fn invite_transitions_from_calling_to_terminated_when_timer_b_fires() {
    let mut ctx = ClientTestContext::setup(SipMethod::Invite).await;

    let opt_err = ctx.client.receive_provisional_response().await.err();

    std::assert_matches!(
        opt_err,
        Some(Error::Transaction(TransactionError::Timeout)),
        "Expected Transaction::Timeout, got {opt_err:?}"
    );

    assert_eq_state!(
        ctx.state,
        State::Terminated,
        "client INVITE must transition to the Terminated state when timer B fires"
    );
}

#[tokio::test]
async fn invite_should_send_ack_when_receiving_3xx_response_in_calling_state() {
    let ctx = ClientTestContext::setup(SipMethod::Invite).await;

    ctx.server.respond(CODE_301_MOVED_PERMANENTLY).await;

    ctx.client
        .receive_final_response()
        .await
        .expect("Error receiving final response");

    let req = ctx.transport.get_last_sent_request().expect("A request");
    assert_eq!(
        req.req_line.method,
        SipMethod::Ack,
        "client INVITE must generate an ACK request when receiving 3xx response"
    );
}

#[tokio::test]
async fn invite_should_send_ack_when_receiving_4xx_response_in_calling_state() {
    let ctx = ClientTestContext::setup(SipMethod::Invite).await;

    ctx.server.respond(CODE_404_NOT_FOUND).await;

    ctx.client
        .receive_final_response()
        .await
        .expect("Error receiving final response");

    let req = ctx.transport.get_last_sent_request().expect("A request");
    assert_eq!(
        req.req_line.method,
        SipMethod::Ack,
        "client INVITE must generate an ACK request when receiving 4xx response"
    );
}

#[tokio::test]
async fn invite_should_send_ack_when_receiving_5xx_response_in_calling_state() {
    let ctx = ClientTestContext::setup(SipMethod::Invite).await;

    ctx.server.respond(CODE_504_SERVER_TIMEOUT).await;

    ctx.client
        .receive_final_response()
        .await
        .expect("Error receiving final response");

    let req = ctx.transport.get_last_sent_request().expect("A request");
    assert_eq!(
        req.req_line.method,
        SipMethod::Ack,
        "client INVITE must generate an ACK request when receiving 5xx response"
    );
}

#[tokio::test]
async fn invite_should_send_ack_when_receiving_6xx_response_in_calling_state() {
    let ctx = ClientTestContext::setup(SipMethod::Invite).await;

    ctx.server.respond(CODE_603_DECLINE).await;

    ctx.client
        .receive_final_response()
        .await
        .expect("Error receiving final response");

    let req = ctx.transport.get_last_sent_request().expect("A request");
    assert_eq!(
        req.req_line.method,
        SipMethod::Ack,
        "client INVITE must generate an ACK request when receiving 6xx response"
    );
}

#[tokio::test]
async fn invite_transitions_from_proceeding_to_completed_when_receiving_3xx_response() {
    let mut ctx = ClientTestContext::setup(SipMethod::Invite).await;

    ctx.server.respond(CODE_301_MOVED_PERMANENTLY).await;

    ctx.client
        .receive_final_response()
        .await
        .expect("Error receiving final response");

    assert_eq_state!(
        ctx.state,
        State::Completed,
        "client INVITE must transition to the Completed state when receiving 3xx response"
    );
}

#[tokio::test]
async fn invite_transitions_from_proceeding_to_completed_when_receiving_4xx_response() {
    let mut ctx = ClientTestContext::setup(SipMethod::Invite).await;

    ctx.server.respond(CODE_404_NOT_FOUND).await;

    ctx.client
        .receive_final_response()
        .await
        .expect("Error receiving final response");

    assert_eq_state!(
        ctx.state,
        State::Completed,
        "client INVITE must transition to the Completed state when receiving 4xx response"
    );
}

#[tokio::test]
async fn invite_transitions_from_proceeding_to_completed_when_receiving_5xx_response() {
    let mut ctx = ClientTestContext::setup(SipMethod::Invite).await;

    ctx.server.respond(CODE_504_SERVER_TIMEOUT).await;

    ctx.client
        .receive_final_response()
        .await
        .expect("Error receiving final response");

    assert_eq_state!(
        ctx.state,
        State::Completed,
        "client INVITE must transition to the Completed state when receiving 5xx response"
    );
}

#[tokio::test]
async fn invite_transitions_from_proceeding_to_completed_when_receiving_6xx_response() {
    let mut ctx = ClientTestContext::setup(SipMethod::Invite).await;

    ctx.server.respond(CODE_603_DECLINE).await;

    ctx.client
        .receive_final_response()
        .await
        .expect("Error receiving final response");

    assert_eq_state!(
        ctx.state,
        State::Completed,
        "client INVITE must transition to the Completed state when receiving 6xx response"
    );
}

#[tokio::test]
async fn invite_transitions_from_proceeding_to_terminated_when_receiving_2xx_response() {
    let mut ctx = ClientTestContext::setup(SipMethod::Invite).await;

    ctx.server.respond(CODE_202_ACCEPTED).await;

    ctx.client
        .receive_final_response()
        .await
        .expect("Error receiving final response");

    assert_eq_state!(
        ctx.state,
        State::Terminated,
        "client INVITE must transition to the Completed state when receiving receiving 2xx response"
    );
}

#[tokio::test(start_paused = true)]
async fn invite_should_not_retransmit_request_in_proceeding_state() {
    let mut ctx = ClientTestContext::setup(SipMethod::Invite).await;
    let expected_requests = 1;
    let expected_retrans = 0;

    ctx.server.respond(CODE_100_TRYING).await;

    ctx.client
        .receive_provisional_response()
        .await
        .expect("Error receiving provisional response")
        .expect("Expected provisional response, but received None");

    assert_eq_state!(
        ctx.state,
        State::Proceeding,
        "client INVITE must transition to the Proceeding state when receiving receiving 1xx response"
    );

    ctx.timer.wait_for_retransmissions(5).await;

    assert_eq!(
        ctx.transport.sent_count(),
        expected_requests + expected_retrans
    );
}

#[tokio::test]
async fn invite_should_send_ack_when_receiving_3xx_response_in_proceeding_state() {
    let mut ctx = ClientTestContext::setup(SipMethod::Invite).await;

    ctx.server.respond(CODE_100_TRYING).await;

    ctx.client
        .receive_provisional_response()
        .await
        .expect("Error receiving provisional response")
        .expect("Expected provisional response, but received None");

    assert_eq_state!(
        ctx.state,
        State::Proceeding,
        "client INVITE must transition to the Proceeding state when receiving receiving 1xx response"
    );

    ctx.server.respond(CODE_301_MOVED_PERMANENTLY).await;

    ctx.client
        .receive_final_response()
        .await
        .expect("Error receiving final response");

    let req = ctx.transport.get_last_sent_request().expect("A request");
    assert_eq!(
        req.req_line.method,
        SipMethod::Ack,
        "client INVITE must generate an ACK request when receiving 3xx response"
    );
}

#[tokio::test]
async fn invite_should_send_ack_after_4xx_response_in_proceeding_state() {
    let mut ctx = ClientTestContext::setup(SipMethod::Invite).await;

    ctx.server.respond(CODE_100_TRYING).await;

    ctx.client
        .receive_provisional_response()
        .await
        .expect("Error receiving provisional response")
        .expect("Expected provisional response, but received None");

    assert_eq_state!(
        ctx.state,
        State::Proceeding,
        "client INVITE must transition to the Proceeding state when receiving receiving 1xx response"
    );

    ctx.server.respond(CODE_404_NOT_FOUND).await;

    ctx.client
        .receive_final_response()
        .await
        .expect("Error receiving final response");

    let req = ctx.transport.get_last_sent_request().expect("A request");
    assert_eq!(
        req.req_line.method,
        SipMethod::Ack,
        "client INVITE must generate an ACK request when receiving 4xx response"
    );
}

#[tokio::test]
async fn invite_should_send_ack_when_receiving_5xx_response_in_proceeding_state() {
    let mut ctx = ClientTestContext::setup(SipMethod::Invite).await;

    ctx.server.respond(CODE_100_TRYING).await;

    ctx.client
        .receive_provisional_response()
        .await
        .expect("Error receiving provisional response")
        .expect("Expected provisional response, but received None");

    assert_eq_state!(
        ctx.state,
        State::Proceeding,
        "client INVITE must transition to the Proceeding state when receiving receiving 1xx response"
    );

    ctx.server.respond(CODE_504_SERVER_TIMEOUT).await;

    ctx.client
        .receive_final_response()
        .await
        .expect("Error receiving final response");

    let req = ctx.transport.get_last_sent_request().expect("A request");
    assert_eq!(
        req.req_line.method,
        SipMethod::Ack,
        "client INVITE must generate an ACK request when receiving 5xx response"
    );
}

#[tokio::test]
async fn invite_should_send_ack_after_6xx_response_in_proceeding_state() {
    let mut ctx = ClientTestContext::setup(SipMethod::Invite).await;

    ctx.server.respond(CODE_100_TRYING).await;

    ctx.client
        .receive_provisional_response()
        .await
        .expect("Error receiving provisional response")
        .expect("Expected provisional response, but received None");

    assert_eq_state!(
        ctx.state,
        State::Proceeding,
        "client INVITE must transition to the Proceeding state when receiving receiving 1xx response"
    );

    ctx.server.respond(CODE_603_DECLINE).await;

    ctx.client
        .receive_final_response()
        .await
        .expect("Error receiving final response");

    let req = ctx.transport.get_last_sent_request().expect("A request");
    assert_eq!(
        req.req_line.method,
        SipMethod::Ack,
        "client INVITE must generate an ACK request when receiving 6xx response"
    );
}

#[tokio::test]
async fn invite_should_pass_provisional_responses_to_tu_in_proceeding_state() {
    let mut ctx = ClientTestContext::setup(SipMethod::Invite).await;

    ctx.server.respond(CODE_100_TRYING).await;
    ctx.server.respond(CODE_180_RINGING).await;

    let incoming = ctx
        .client
        .receive_provisional_response()
        .await
        .expect("Error receiving provisional response")
        .expect("Expected provisional response, but received None");

    assert_eq!(
        incoming.response.status_line.code, CODE_100_TRYING,
        "should match 100 status code"
    );

    let incoming = ctx
        .client
        .receive_provisional_response()
        .await
        .expect("Error receiving provisional response")
        .expect("Expected provisional response, but received None");

    assert_eq!(
        incoming.response.status_line.code, CODE_180_RINGING,
        "should match 180 status code"
    );
}

#[tokio::test(start_paused = true)]
async fn invite_transitions_from_completed_to_terminated_when_timer_d_fires() {
    let mut ctx = ClientTestContext::setup(SipMethod::Invite).await;

    ctx.server.respond(CODE_301_MOVED_PERMANENTLY).await;

    ctx.client
        .receive_final_response()
        .await
        .expect("Error receiving final response");

    assert_eq_state!(
        ctx.state,
        State::Completed,
        "client INVITE must transition to the Completed state when receiving 3xx response"
    );

    ctx.timer.timer_d().await;

    assert_eq_state!(
        ctx.state,
        State::Terminated,
        "client INVITE must transition to the Terminated state when timer D fires"
    );
}

// Non-INVITE Client tests

#[tokio::test]
async fn non_invite_transitions_to_trying_when_request_is_sent() {
    let ctx = SendRequestContext::setup(SipMethod::Register).await;

    let uac = ClientTransaction::send_request_with_target(
        ctx.request,
        ctx.endpoint,
        (ctx.transport, ctx.destination),
    )
    .await
    .expect("failure sending request");

    assert_eq!(
        uac.state(),
        State::Trying,
        "client non-INVITE must transition to the Trying state when sending the request"
    );
}

#[tokio::test(start_paused = true)]
async fn non_invite_should_not_start_timer_e_when_transport_is_reliable() {
    let mut ctx = ClientTestContext::setup_reliable(SipMethod::Invite).await;
    let expected_requests = 1;
    let expected_retrans = 0;

    let opt_err = ctx.client.receive_provisional_response().await.err();

    std::assert_matches!(
        opt_err,
        Some(Error::Transaction(TransactionError::Timeout)),
        "Expected Transaction::Timeout, got {opt_err:?}"
    );

    assert_eq!(
        ctx.transport.sent_count(),
        expected_requests + expected_retrans,
        "sent count should match {expected_requests} requests and {expected_retrans} retransmissions"
    );
}

#[tokio::test]
async fn non_invite_transitions_from_trying_to_proceeding_when_receiving_1xx_response() {
    let mut ctx = ClientTestContext::setup(SipMethod::Register).await;

    ctx.server.respond(CODE_100_TRYING).await;

    ctx.client
        .receive_provisional_response()
        .await
        .expect("Error receiving provisional response")
        .expect("Expected provisional response, but received None");

    assert_eq_state!(
        ctx.state,
        State::Proceeding,
        "client non-INVITE must transition to the Proceeding state when receiving receiving 1xx response"
    );
}

#[tokio::test]
async fn non_invite_transitions_from_trying_to_completed_when_receiving_2xx_response() {
    let mut ctx = ClientTestContext::setup(SipMethod::Options).await;

    ctx.server.respond(CODE_301_MOVED_PERMANENTLY).await;

    ctx.client
        .receive_final_response()
        .await
        .expect("Error receiving final response");

    assert_eq_state!(
        ctx.state,
        State::Completed,
        "client non-INVITE must transition to the Completed state when receiving receiving 6xx response"
    );
}

#[tokio::test]
async fn non_invite_transitions_from_trying_to_completed_when_receiving_3xx_response() {
    let mut ctx = ClientTestContext::setup(SipMethod::Options).await;

    ctx.server.respond(CODE_301_MOVED_PERMANENTLY).await;

    ctx.client
        .receive_final_response()
        .await
        .expect("Error receiving final response");

    assert_eq_state!(
        ctx.state,
        State::Completed,
        "client non-INVITE must transition to the Completed state when receiving receiving 3xx response"
    );
}

#[tokio::test]
async fn non_invite_transitions_from_trying_to_completed_when_receiving_4xx_response() {
    let mut ctx = ClientTestContext::setup(SipMethod::Options).await;

    ctx.server.respond(CODE_404_NOT_FOUND).await;

    ctx.client
        .receive_final_response()
        .await
        .expect("Error receiving final response");

    assert_eq_state!(
        ctx.state,
        State::Completed,
        "client non-INVITE must transition to the Completed state when receiving receiving 4xx response"
    );
}

#[tokio::test]
async fn non_invite_transitions_from_trying_to_completed_when_receiving_5xx_response() {
    let mut ctx = ClientTestContext::setup(SipMethod::Options).await;

    ctx.server.respond(CODE_504_SERVER_TIMEOUT).await;

    ctx.client
        .receive_final_response()
        .await
        .expect("Error receiving final response");

    assert_eq_state!(
        ctx.state,
        State::Completed,
        "client non-INVITE must transition to the Completed state when receiving receiving 5xx response"
    );
}

#[tokio::test]
async fn non_invite_transitions_from_trying_to_completed_when_receiving_6xx_response() {
    let mut ctx = ClientTestContext::setup(SipMethod::Options).await;

    ctx.server.respond(CODE_603_DECLINE).await;

    ctx.client
        .receive_final_response()
        .await
        .expect("Error receiving final response");

    assert_eq_state!(
        ctx.state,
        State::Completed,
        "client non-INVITE must transition to the Completed state when receiving receiving 6xx response"
    );
}
#[tokio::test]
async fn non_invite_transitions_from_proceeding_to_completed_when_receiving_3xx_response() {
    let mut ctx = ClientTestContext::setup(SipMethod::Options).await;

    ctx.server.respond(CODE_301_MOVED_PERMANENTLY).await;

    ctx.client
        .receive_final_response()
        .await
        .expect("Error receiving final response");

    assert_eq_state!(
        ctx.state,
        State::Completed,
        "client non-INVITE must transition to the Completed state when receiving receiving 3xx response"
    );
}

#[tokio::test]
async fn non_invite_transitions_from_proceeding_to_completed_when_receiving_4xx_response() {
    let mut ctx = ClientTestContext::setup(SipMethod::Options).await;

    ctx.server.respond(CODE_404_NOT_FOUND).await;

    ctx.client
        .receive_final_response()
        .await
        .expect("Error receiving final response");

    assert_eq_state!(
        ctx.state,
        State::Completed,
        "client non-INVITE must transition to the Completed state when receiving receiving 4xx response"
    );
}

#[tokio::test]
async fn non_invite_transitions_from_proceeding_to_completed_when_receiving_5xx_response() {
    let mut ctx = ClientTestContext::setup(SipMethod::Options).await;

    ctx.server.respond(CODE_504_SERVER_TIMEOUT).await;

    ctx.client
        .receive_final_response()
        .await
        .expect("Error receiving final response");

    assert_eq_state!(
        ctx.state,
        State::Completed,
        "client non-INVITE must transition to the Completed state when receiving receiving 5xx response"
    );
}

#[tokio::test]
async fn non_invite_transitions_from_proceeding_to_completed_when_receiving_6xx_response() {
    let mut ctx = ClientTestContext::setup(SipMethod::Options).await;

    ctx.server.respond(CODE_603_DECLINE).await;

    ctx.client
        .receive_final_response()
        .await
        .expect("Error receiving final response");

    assert_eq_state!(
        ctx.state,
        State::Completed,
        "client non-INVITE must transition to the Completed state when receiving receiving 6xx response"
    );
}

#[tokio::test]
async fn non_invite_transitions_from_proceeding_to_completed_when_receiving_2xx_response() {
    let mut ctx = ClientTestContext::setup(SipMethod::Options).await;

    ctx.server.respond(CODE_202_ACCEPTED).await;

    ctx.client
        .receive_final_response()
        .await
        .expect("Error receiving final response");

    assert_eq_state!(
        ctx.state,
        State::Completed,
        "client non-INVITE must transition to the Completed state when receiving receiving 2xx response"
    );
}

#[tokio::test(start_paused = true)]
async fn non_invite_transitions_from_trying_to_terminated_when_timer_f_fires() {
    let mut ctx = ClientTestContext::setup(SipMethod::Register).await;

    let opt_err = ctx.client.receive_provisional_response().await.err();

    std::assert_matches!(
        opt_err,
        Some(Error::Transaction(TransactionError::Timeout)),
        "Expected Transaction::Timeout, got {opt_err:?}"
    );

    assert_eq_state!(
        ctx.state,
        State::Terminated,
        "client non-INVITE must transition to the Terminated state when timer F fires"
    );
}

#[tokio::test(start_paused = true)]
async fn non_invite_should_not_retransmit_request_in_proceeding_state() {
    let mut ctx = ClientTestContext::setup(SipMethod::Options).await;
    let expected_requests = 1;
    let expected_retrans = 0;

    ctx.server.respond(CODE_100_TRYING).await;

    ctx.client
        .receive_provisional_response()
        .await
        .expect("Error receiving provisional response")
        .expect("Expected provisional response, but received None");

    assert_eq_state!(
        ctx.state,
        State::Proceeding,
        "client non-INVITE must transition to the Proceeding state when receiving receiving 1xx response"
    );

    ctx.timer.wait_for_retransmissions(5).await;

    assert_eq!(
        ctx.transport.sent_count(),
        expected_requests + expected_retrans,
        "sent count should match {expected_requests} requests and {expected_retrans} retransmissions"
    );
}

#[tokio::test]
async fn non_invite_should_pass_provisional_responses_to_tu_in_proceeding_state() {
    let mut ctx = ClientTestContext::setup(SipMethod::Options).await;

    ctx.server.respond(CODE_100_TRYING).await;
    ctx.server.respond(CODE_180_RINGING).await;

    let response = ctx
        .client
        .receive_provisional_response()
        .await
        .expect("Error receiving provisional response")
        .expect("Expected provisional response, but received None");

    assert_eq!(
        response.response.status_line.code, CODE_100_TRYING,
        "should match 100 status code"
    );

    let response = ctx
        .client
        .receive_provisional_response()
        .await
        .expect("Error receiving provisional response")
        .expect("Expected provisional response, but received None");

    assert_eq!(
        response.response.status_line.code, CODE_180_RINGING,
        "should match 180 status code"
    );
}

#[tokio::test(start_paused = true)]
async fn non_invite_transitions_from_completed_to_terminated_when_timer_k_fires() {
    let mut ctx = ClientTestContext::setup(SipMethod::Options).await;

    ctx.server.respond(CODE_301_MOVED_PERMANENTLY).await;

    ctx.client
        .receive_final_response()
        .await
        .expect("Error receiving final response");

    assert_eq_state!(
        ctx.state,
        State::Completed,
        "client non-INVITE must transition to the Completed state when receiving receiving 3xx response"
    );

    ctx.timer.timer_k().await;
    tokio::task::yield_now().await;

    assert_eq_state!(
        ctx.state,
        State::Terminated,
        "should transition to Terminated after timer d fires"
    );
}
