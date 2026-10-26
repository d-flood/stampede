use crate::common;

use common::{Server, View};

#[tokio::test]
async fn a_title_set_by_a_program_clears_when_the_shell_takes_the_foreground_back() {
    let server = Server::start("title").await;
    let mut view = View::open(server.port, "device", None).await;
    view.settle(|text| text.contains('$')).await;
    assert_eq!(view.label, "bash — ~");

    view.send("sh -c 'printf \"\\033]0;PROGRAM\\007\"; read -r _'\r").await;
    view.wait(|view| view.label == "PROGRAM").await;
    view.send("\r").await;
    view.wait(|view| view.label == "bash — ~").await;

    view.send("printf '\\033]0;SHELL\\007'; head -n 1\r").await;
    view.wait(|view| view.label == "SHELL").await;
    view.send("\r").await;
    view.wait(|view| view.label == "bash — ~").await;
}
