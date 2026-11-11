# Scenario of the typing test (fs-release actions/typing-test): defines focus(), which brings a text field of the window
# into focus. The window opens on the list of groups; its search field ("Search groups") takes the text. Positions are
# relative to the window's top left corner (WX0, WY0, set by the test; title bar included), for the window size of the
# workflow (`--size 1000x640`): the field is at (170, 74) in the picture of the window, y=88 hits the field with and without a title bar (as in fs-people).
focus() {
  local x=$((WX0 + 170)) y=$((WY0 + 88))
  TT_NEAR="$x,$y"; export TT_NEAR
  c "move $x $y" 0.4
  c "click $x $y" 1
}
