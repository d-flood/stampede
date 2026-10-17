# Manual checklist

Run at milestones against an installed Oikonomos (`oikonomos install`), opened at its tailnet URL.

## 1. Real Agents: status and recovery

- [ ] Launch a Claude Code Rig. Its Chat tab shows _working_ during a turn, _waiting_ at a permission prompt or question, and _done_ when the turn finishes.
- [ ] Do the same with a Codex Rig.
- [ ] Reboot the Host. Each Chat restores its Agent session, with an interrupted turn marked for attention.

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

- [ ] Open a Terminal and a Chat, each with output on screen, then reboot the Host without logging in.
- [ ] The tailnet URL loads after the reboot.
- [ ] The Terminal returns as an Ended Terminal showing its last output; the Chat restores its transcript and session.
- [ ] Sending a new message continues the Chat; dismissing the Ended Terminal removes it.

## 5. Chats

Use a throwaway Project and repeat each applicable item with Claude Code and Codex. Check both desktop and the installed Pixel PWA.

- [ ] Every Rig opens a Chat from New Tab, the Palette and shortcuts, on desktop and phone; model, effort, mode and account match the Rig.
- [ ] Start a TUI manually in a Terminal; it receives no Oikonomos hooks, status, attention, resume or handoff controls.
- [ ] A running turn shows working.
- [ ] A pending request shows waiting.
- [ ] A finished turn shows done.
- [ ] An unseen waiting or finished Chat appears in Attention and sends a push; tapping it opens the Chat.
- [ ] Allow a command approval from its docked card.
- [ ] Allow an edit approval from its docked card.
- [ ] Allow an approval for another tool from its docked card.
- [ ] Deny an approval; the Agent stops that action.
- [ ] Type in the Composer during an approval; the Agent receives the denial and instruction.
- [ ] Choose Always allow, shorten a command prefix, and keep This Chat scope. Claude reuses the prefix; Codex offers an exact-command session allowance.
- [ ] Choose Always scope with a shorter prefix; a new session reuses it.
- [ ] Approve a compound command with a separate rule for each part. For Codex, restart Codex before checking the additional persistent rules.
- [ ] Answer a question by tapping an option.
- [ ] Answer a question with Other.
- [ ] Select several answers on a multiple-choice question.
- [ ] Step through a multi-question card and submit; the Agent receives every answer.
- [ ] Skip a question.
- [ ] Type in the Composer during a question; the Agent receives the free-text answer.
- [ ] Approve a proposed plan; the Agent leaves plan mode and starts implementing.
- [ ] Choose Keep planning; the Agent remains in plan mode.
- [ ] Answer a pending request from one Device; the card clears on the other Device.
- [ ] Steer a running turn from the Composer.
- [ ] Queue a message; it starts only after the current turn finishes.
- [ ] Interrupt a turn; queued text returns to the Composer.
- [ ] Attach a photo from the Pixel camera; its thumbnail appears and the Agent can describe it.
- [ ] Attach a photo from the Pixel photo picker.
- [ ] Remove an attachment before sending.
- [ ] An image larger than 5 MB is refused with a message.
- [ ] `/model`, `/effort` and `/mode` update their chips and take effect.
- [ ] `/compact` reduces context fill; `/clear` starts a fresh conversation in the same Tab.
- [ ] An edit card opens its file in the Diff tab.
- [ ] Claude `/rewind` restores an Edit/Write change and removes the chosen prompt and later transcript; Bash and subagent changes remain.
- [ ] Codex `/rewind` removes the selected turn and later transcript, leaves files unchanged, and says so.
- [ ] Restart Oikonomos mid-turn; the Chat continues without missing transcript.
- [ ] Restart Oikonomos at a question or approval; the request can still be answered.
- [ ] Reboot the Host mid-turn; the session returns without re-running the prompt, marked interrupted and in Attention.
- [ ] Compare Claude five-hour and weekly meters with its TUI usage display.
- [ ] Compare Codex five-hour and weekly meters with its TUI usage display.
- [ ] Meter ages advance and reset when readings refresh; unavailable windows show no invented value.
- [ ] Reopen a past session from New Tab and from Search sessions; its history appears.

Run results and unavailable checks: `epic/chats/22-manual-checklist.md`.
