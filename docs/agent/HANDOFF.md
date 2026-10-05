# Handoff

## Context
We just designed the `ControlProcessor` trait and the `MpeInstrument` state for MPE voice allocation. It uses an `[Option<ActiveVoice>; 15]` array to track active touches without dynamic memory allocation or redundant channel data.

## Next Action
Implement the logic inside `MpeInstrument::process_event`. This involves:
- Voice Stealing / Allocation: Finding a free `None` slot for new `Touch` events.
- Routing: Finding the matching `touch_id` for incoming `Position` or `Released` events.
- Generating the appropriate `InstrumentEvent`s into the output buffer.

## Verification
The workspace is green, CI is configured and passing.
