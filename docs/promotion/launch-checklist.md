# Launch checklist

Do these before posting the first announcement.

## Repo housekeeping

- [ ] Decide what to do about the 0.2.1 release: tag `v0.2.1` is on the hotfix
      branch (PR #25, still open), while `main` is still at 0.2.0. Merge or
      reconcile before telling people "main is 0.2.1".
- [ ] Update the README version badge — it still says `v0.1.9`.
- [ ] Update `CONTRIBUTING.md` prerequisites — it says Rust 1.90+ / Node 20+,
      but the repo pins Rust 1.100.0 and requires Node 24+.
- [ ] Add a `Contributing` link and the `docs/promotion/` index to the README.
- [ ] Create the `good first issue` and `help wanted` labels, then open two or
      three of the scoped issues from `contributors.md`.

## Assets

- [ ] One screenshot of chat with citation pills (the feature people remember).
- [ ] One screenshot of the document library search screen.
- [ ] A 15–30 s clip: load a local model, ask a question over an indexed folder,
      show the citation. No narration needed; captions are better.
- [ ] Confirm the repo description and topics on GitHub match `pitch.md`.

## Verification before claiming anything

- [ ] Confirm the release you link is the one you tested (v0.2.1 assets exist).
- [ ] Confirm the caveats in `pitch.md` still hold after the hotfix merge.
- [ ] Re-read the "honesty rules" in `docs/promotion/README.md` once, right
      before posting.

## Posting order

1. GitHub repo cleanup above.
2. Show HN in the morning; stay present in comments for two hours.
3. r/LocalLLaMA a day or two later (local-model angle).
4. Social thread the same week; contributor post a week later.
5. r/opensource / r/programming only if the subs allow project posts.

## After posting

- Track where signups/stars/issues come from (a simple note in a spreadsheet
  is enough). Ask every new user one question: "what almost stopped you from
  trying it?" That answer drives the README fix.
- Reply to every issue and PR quickly during the first month.
- Update `docs/evidence/scorecard.md` with achieved numbers, not targets, as
  evidence lands; the scorecard currently says its numbers are targets only.
