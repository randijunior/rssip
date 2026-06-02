use super::*;
use crate::assert_eq_state;
use crate::test_utils::transaction::{
    CODE_100_TRYING, CODE_202_ACCEPTED, CODE_301_MOVED_PERMANENTLY, CODE_504_SERVER_TIMEOUT,
    ServerTestContext,
};

// INVITE Server tests

#[tokio::test]
async fn invite_transitions_to_proceeding_when_created_from_request() {
    let ctx = ServerTestContext::setup(SipMethod::Invite).await;

    assert_eq!(
        ctx.server.state_machine.state(),
        State::Proceeding,
        "server INVITE must transition to the Proceeding state when constructed for a request"
    );
}

#[tokio::test]
async fn invite_transitions_to_confirmed_when_receiving_ack() {
    let mut ctx = ServerTestContext::setup(SipMethod::Invite).await;

    ctx.server
        .send_final_status(CODE_301_MOVED_PERMANENTLY)
        .await
        .expect("Error sending final response");

    assert_eq_state!(
        ctx.state,
        State::Completed,
        "server INVITE must transition to the Completed state when sending 300-699 response"
    );

    ctx.client.send_ack_request().await;

    assert_eq_state!(
        ctx.state,
        State::Confirmed,
        "server INVITE must transition to the Confirmed state when receiving the ACK request"
    );
}

#[tokio::test]
async fn invite_unreliable_transitions_to_terminated_when_sending_2xx_response() {
    let mut ctx = ServerTestContext::setup(SipMethod::Invite).await;

    ctx.server
        .send_final_status(CODE_202_ACCEPTED)
        .await
        .expect("Error sending final response");

    assert_eq_state!(
        ctx.state,
        State::Terminated,
        "server INVITE must transition to the Terminated state when sending 2xx response"
    );
}

#[tokio::test]
async fn invite_reliable_transitions_to_terminated_when_sending_2xx_response() {
    let mut ctx = ServerTestContext::setup_reliable(SipMethod::Invite).await;

    ctx.server
        .send_final_status(CODE_202_ACCEPTED)
        .await
        .expect("Error sending final response");

    assert_eq_state!(
        ctx.state,
        State::Terminated,
        "server INVITE must transition to the Terminated state when sending 2xx response"
    );
}

#[tokio::test]
async fn invite_should_retransmit_response_when_receiving_request_retransmission() {
    let ctx = ServerTestContext::setup(SipMethod::Invite).await;
    let expected_responses = 1;
    let expected_retrans = 3;

    ctx.server
        .send_final_status(CODE_301_MOVED_PERMANENTLY)
        .await
        .expect("Error sending final response");

    ctx.client.retransmit_n_times(expected_retrans).await;

    assert_eq!(
        ctx.transport.sent_count(),
        expected_responses + expected_retrans,
        "sent count should match {expected_responses} responses and {expected_retrans} retransmissions"
    );
}

#[tokio::test(start_paused = true)]
async fn invite_must_cease_retransmission_when_receiving_ack() {
    let mut ctx = ServerTestContext::setup(SipMethod::Invite).await;
    let expected_responses = 1;
    let expected_retrans = 2;

    ctx.server
        .send_final_status(CODE_301_MOVED_PERMANENTLY)
        .await
        .expect("Error sending final response");

    ctx.timer.wait_for_retransmissions(expected_retrans).await;

    ctx.client.send_ack_request().await;

    // Should not retransmit at this point.
    ctx.timer.wait_for_retransmissions(3).await;

    assert_eq!(
        ctx.transport.sent_count(),
        expected_responses + expected_retrans,
        "sent count should match {expected_responses} responses and {expected_retrans} retransmissions"
    );
}

#[tokio::test(start_paused = true)]
async fn invite_timer_h_must_be_set_for_reliable_transports() {
    let mut ctx = ServerTestContext::setup_reliable(SipMethod::Invite).await;

    ctx.server
        .send_final_status(CODE_301_MOVED_PERMANENTLY)
        .await
        .expect("Error sending final response");

    assert_eq_state!(
        ctx.state,
        State::Completed,
        "server INVITE must transition to the Completed state when sending final 200-699 response"
    );

    ctx.timer.timer_h().await;

    assert_eq_state!(
        ctx.state,
        State::Terminated,
        "server INVITE must transition to the Terminated state when timer H fires"
    );
}

#[tokio::test(start_paused = true)]
async fn invite_timer_h_must_be_set_for_unreliable_transports() {
    let mut ctx = ServerTestContext::setup(SipMethod::Invite).await;

    ctx.server
        .send_final_status(CODE_301_MOVED_PERMANENTLY)
        .await
        .expect("Error sending final response");

    assert_eq_state!(
        ctx.state,
        State::Completed,
        "server INVITE must transition to the Completed state when sending 200-699 response"
    );

    ctx.timer.timer_h().await;

    assert_eq_state!(
        ctx.state,
        State::Terminated,
        "server INVITE must transition to the Terminated state when timer H fires"
    );
}

#[tokio::test(start_paused = true)]
async fn invite_transitions_to_terminated_when_timer_i_fires() {
    let mut ctx = ServerTestContext::setup(SipMethod::Invite).await;

    ctx.server
        .send_final_status(CODE_301_MOVED_PERMANENTLY)
        .await
        .expect("Error sending final response");

    assert_eq_state!(
        ctx.state,
        State::Completed,
        "server INVITE must must transition to the Completed when sending 300-699 response"
    );

    ctx.client.send_ack_request().await;

    assert_eq_state!(
        ctx.state,
        State::Confirmed,
        "server INVITE must transition to the Confirmed state when receiving ACK request"
    );

    ctx.timer.timer_i().await;

    assert_eq_state!(
        ctx.state,
        State::Terminated,
        "server INVITE must transition to the Terminated state when timer I fires"
    );
}

#[tokio::test(start_paused = true)]
async fn invite_retransmit_response_when_timer_g_fires() {
    let mut ctx = ServerTestContext::setup(SipMethod::Invite).await;
    let expected_responses = 1;
    let expected_retrans = 3;

    ctx.server
        .send_final_status(CODE_301_MOVED_PERMANENTLY)
        .await
        .expect("Error sending final response");

    ctx.timer.wait_for_retransmissions(expected_retrans).await;

    assert_eq!(
        ctx.transport.sent_count(),
        expected_responses + expected_retrans,
        "sent count should match {expected_responses} requests and {expected_retrans} retransmissions"
    );
}

// Non-INVITE Server tests

#[tokio::test]
async fn non_invite_transitions_to_trying_when_created_from_request() {
    let ctx = ServerTestContext::setup(SipMethod::Options).await;

    assert_eq!(
        ctx.server.state_machine.state(),
        State::Trying,
        "server non-INVITE must transition to the Trying state when constructed for a request"
    );
}

#[tokio::test]
async fn non_invite_transition_to_proceeding_when_sending_1xx_response() {
    let mut ctx = ServerTestContext::setup(SipMethod::Options).await;

    ctx.server
        .send_provisional_status(CODE_100_TRYING)
        .await
        .expect("Error sending provisional response");

    assert_eq_state!(
        ctx.state,
        State::Proceeding,
        "server non-INVITE must transition to the Proceeding state when sending 1xx response"
    );
}

#[tokio::test]
async fn non_invite_transition_to_completed_when_sending_non_2xx_response() {
    let mut ctx = ServerTestContext::setup(SipMethod::Options).await;

    ctx.server
        .send_final_status(CODE_504_SERVER_TIMEOUT)
        .await
        .expect("Error sending final response");

    assert_eq_state!(
        ctx.state,
        State::Completed,
        "server non-INVITE must transition to the Completed state when sending 200-699 response"
    );
}

#[tokio::test]
async fn non_invite_reliable_transition_to_terminated_when_sending_2xx_response() {
    let mut ctx = ServerTestContext::setup_reliable(SipMethod::Options).await;

    ctx.server
        .send_final_status(CODE_202_ACCEPTED)
        .await
        .expect("Error sending final response");

    assert_eq_state!(
        ctx.state,
        State::Completed,
        "server non-INVITE must transition to the Completed when sending 200-699 response"
    );

    assert_eq_state!(
        ctx.state,
        State::Terminated,
        "server non-INVITE must transition to the Terminated state when sending 2xx response"
    );
}

#[tokio::test]
async fn non_invite_reliable_transition_to_terminated_when_sending_non_2xx_response() {
    let mut ctx = ServerTestContext::setup_reliable(SipMethod::Options).await;

    ctx.server
        .send_final_status(CODE_504_SERVER_TIMEOUT)
        .await
        .expect("Error sending final response");

    assert_eq_state!(
        ctx.state,
        State::Completed,
        "server non-INVITE must transition to the Completed when sending 200-699 response"
    );

    assert_eq_state!(
        ctx.state,
        State::Terminated,
        "server non-INVITE must transition to the Terminated state when sending 2xx response"
    );
}

#[tokio::test]
async fn non_invite_absorbs_retransmission_in_trying_state() {
    let ctx = ServerTestContext::setup(SipMethod::Options).await;
    let expected_retrans = 0;

    ctx.client.retransmit_n_times(2).await;

    assert_eq!(
        ctx.transport.sent_count(),
        expected_retrans,
        "sent count should match {expected_retrans} retransmissions"
    );
}

#[tokio::test]
async fn non_invite_retransmit_provisional_response_when_receiving_request_retransmission() {
    let mut ctx = ServerTestContext::setup(SipMethod::Options).await;
    let expected_responses = 1;
    let expected_retrans = 4;

    ctx.server
        .send_provisional_status(CODE_100_TRYING)
        .await
        .expect("Error sending provisional response");

    ctx.client.retransmit_n_times(expected_retrans).await;

    assert_eq!(
        ctx.transport.sent_count(),
        expected_responses + expected_retrans,
        "sent count should match {expected_responses} responses and {expected_retrans} retransmissions"
    );
}

#[tokio::test]
async fn non_invite_retransmit_final_response_when_receiving_request_retransmission() {
    let ctx = ServerTestContext::setup(SipMethod::Register).await;
    let expected_responses = 1;
    let expected_retrans = 2;

    ctx.server
        .send_final_status(CODE_202_ACCEPTED)
        .await
        .expect("Error sending final response");

    ctx.client.retransmit_n_times(expected_retrans).await;

    assert_eq!(
        ctx.transport.sent_count(),
        expected_responses + expected_retrans,
        "sent count should match {expected_responses} responses and {expected_retrans} retransmissions"
    );
}

#[tokio::test(start_paused = true)]
async fn non_invite_transitions_to_terminated_when_timer_j_fires() {
    let mut ctx = ServerTestContext::setup(SipMethod::Bye).await;

    ctx.server
        .send_final_status(CODE_202_ACCEPTED)
        .await
        .expect("Error sending final response");

    assert_eq_state!(
        ctx.state,
        State::Completed,
        "server non-INVITE must must transition to the Completed state when sending 200-699 response"
    );

    ctx.timer.timer_j().await;

    assert_eq_state!(
        ctx.state,
        State::Terminated,
        "server non-INVITE must transition to the Terminated state when timer J fires"
    );
}
