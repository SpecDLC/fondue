# Fondue acceptance criteria (DRAFT)

**Status: draft, not ratified.** These cards were drafted from the prototype's README and design notes so the demo has a written contract before it ships. Each one is a proposal until it is marked ratified.

One falsifiable claim per card:

- **Claim**: what must hold
- **Interface**: the only observations a check may use
- **Forbids**: the counterexample that refutes it
- **Done when**: how we know it holds

| ID | Claim | Source | Status |
|---|---|---|---|
| [FONDUE-ENG-1](FONDUE-ENG-1.md) | Content ids are canonical and the stored log round-trips | prototype tests | draft |
| [FONDUE-ENG-2](FONDUE-ENG-2.md) | Malformed events are refused and leave no trace | prototype tests | draft |
| [FONDUE-ENG-3](FONDUE-ENG-3.md) | Levels are derived from nesting, never stored | prototype tests | draft |
| [FONDUE-ENG-4](FONDUE-ENG-4.md) | An open slot asks the cook once | prototype tests | draft |
| [FONDUE-ENG-5](FONDUE-ENG-5.md) | The pantry only replaces a default of the same kind | prototype tests | draft |
| [FONDUE-ENG-6](FONDUE-ENG-6.md) | Contracts follow what a bound thing is | prototype tests | draft |
| [FONDUE-ENG-7](FONDUE-ENG-7.md) | Composition cycles are refused at every entry point | prototype tests | draft |
| [FONDUE-SITE-1](FONDUE-SITE-1.md) | The demo page is one self-contained file built from source | build script | draft |
| [FONDUE-SITE-2](FONDUE-SITE-2.md) | The engine on the page answers fast enough to feel instant | prototype README | draft |
| [FONDUE-SITE-3](FONDUE-SITE-3.md) | The page is served at fondue.specdlc.com over HTTPS | release plan | draft, proposed |
| [FONDUE-SITE-4](FONDUE-SITE-4.md) | The page sets no cookies and sends nothing about the visitor | release plan | draft, proposed |
