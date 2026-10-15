# Manual checklist

Run at milestones against an installed Oikonomos (`oikonomos install`), opened at its tailnet URL.

## 1. Real Agents: status and Resume

- [ ] Launch a Claude Code Rig. Its Agent tab shows _working_ during a turn, _waiting_ at a permission prompt or question, and _done_ when the turn finishes.
- [ ] Do the same with a Codex Rig.
- [ ] Reboot the Host. Each Agent tab returns as an Ended Terminal, and Resume continues that Agent's conversation.

## 2. Pixel push notifications

- [ ] On the Pixel, open the tailnet URL in Chrome and install the PWA.
- [ ] With the PWA in the background, an Agent turning _waiting_ and then _done_ each arrive as a push notification.
- [ ] Tapping a notification opens that Tab.
- [ ] No notification arrives on a Device already showing that Tab.

## 3. Gboard typing and the Composer

- [ ] Typing with Gboard straight into a Terminal shows no autocorrect or suggestions, and commands arrive unmangled.
- [ ] Voice typing into the Composer, then Send, delivers the text as one input followed by Enter.
- [ ] Insert pastes without Enter, and a multi-line draft arrives as one input.

## 4. Host reboot

- [ ] Open a plain Terminal and an Agent tab, each with output on screen, then reboot the Host without logging in.
- [ ] The tailnet URL loads after the reboot.
- [ ] Both Tabs are Ended Terminals showing their last output.
- [ ] Resume on the Agent's Ended Terminal continues its conversation, and dismissing an Ended Terminal removes it.
