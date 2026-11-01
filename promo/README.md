# Promo video

Scenes are recorded from the real app with the fake agents, then edited in Remotion.

```sh
./check e2e --config promo/playwright.config.ts   # capture footage into promo/public/footage
pnpm -C promo install
pnpm -C promo music                               # synthesize the soundtrack
pnpm -C promo render                              # write promo/out/promo.mp4
```

- `seed/tidepool` is the demo project; `web/promo/*.spec.ts` stage it, script the agent with `web/promo/chat.ts`, and record each Device.
- `src/edit.ts` sets the tempo and section lengths that both the music and the cut follow.
- `pnpm -C promo studio` previews the edit.
