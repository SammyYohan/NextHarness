<div align="center">

# NextHarness

**One coding task. Every AI coding harness you already have. No quota wasted.**

[![CI](https://github.com/<OWNER>/next-harness/actions/workflows/ci.yml/badge.svg)](https://github.com/<OWNER>/next-harness/actions/workflows/ci.yml)
[![release](https://img.shields.io/github/v/release/<OWNER>/next-harness?include_prereleases)](https://github.com/<OWNER>/next-harness/releases)
[![license](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org)

[English](#english) · [Français](#français)

</div>

---

## English

### Why

Every coding agent gives you a free allowance, and every free allowance runs out
in the middle of a task. Switching by hand means re-reading the diff, retyping
the request, and hoping the next tool understands where the last one stopped.

Orion Relay does that handover. It starts your task with the first harness in
your list, notices when a harness is out of quota, writes down what was done, and
starts the next one on the same repair — with the same task file and the same
working tree.

It is not an agent. It never talks to a model, never reads a token, and never
pretends to be one of the tools it drives.

### Table of contents

- [How it works](#how-it-works)
- [Install](#install)
- [Quick start](#quick-start)
- [Commands](#commands)
- [Why a harness is paused](#why-a-harness-is-paused)
- [Safety rules](#safety-rules)
- [Configuration](#configuration)
- [Compatibility](#compatibility)
- [Honest limits](#honest-limits)
- [What Orion does not do](#what-next-harness-does-not-do)
- [Contributing](#contributing)
- [Support](#support)

### How it works

```text
  you ── next-harness run "add a dark mode" ──▶ .next-harness/TASK.md
                                              │
                        catalog order         ▼
              ┌──────────────────▶ harness #1 (free) ──▶ finished ──▶ done
              │                       │
              │                       │ out of quota
              │                       ▼
              │            state.json: pause #1 until reset (atomic write)
              │            .next-harness/HANDOFF.md: task, git status, diff --stat,
              │                              end of output, "do not redo"
              │                       │
              └───────────────────────┘  next harness gets HANDOFF.md
                                              │
                                    every candidate exhausted
                                              ▼
                          gatekeeper: pay now / wait for reset / stop
```

Detection, the quota decision and the handover are all local. The only network
traffic is the one your harness already makes.

### Install

**Prebuilt binary** — download the archive for your platform from
[Releases](https://github.com/<OWNER>/next-harness-relay/releases), verify the
`.sha256` next to it, and put `next-harness` on your `PATH`.

**From source** — you need Rust 1.85 or newer:

```sh
cargo install --git https://github.com/<OWNER>/next-harness-relay --bin next-harness
```

On Linux the build needs the GTK and XDo headers, because the pay prompt is a
native dialog:

```sh
sudo apt-get install libgtk-3-dev libxdo-dev
```

### Quick start

```sh
cd ~/code/my-project
next-harness init                          # detect what is installed, write defaults
next-harness doctor                        # see the relay order and what is missing
next-harness run "add a dark mode and tests"
```

That is the whole loop. If the first harness runs out of quota, Orion pauses it,
writes `.next-harness/HANDOFF.md`, and starts the next one. When everything is
exhausted it asks: pay, wait for the reset, or stop.

Every run leaves its trail in the repository:

```text
.next-harness/
├── TASK.md      the task, written once
├── HANDOFF.md   rewritten before each switch
└── runs/        per-attempt transcripts (ANSI stripped, secrets masked)
```

### Commands

| Command | What it does |
| --- | --- |
| `next-harness init` | Detect installed harnesses, print the relay order, write `config.json` |
| `next-harness run "<task>"` | Run a task, relaying on quota |
| `next-harness resume` | Continue the task already in `.next-harness/TASK.md` |
| `next-harness status` | Which harnesses are paused, and until when |
| `next-harness doctor [--refresh] [--report]` | Inventory; `--report` prints pasteable Markdown |
| `next-harness config` | Where the configuration and state live |

Useful flags: `--lang fr|en` on any command, `--max-relays <n>` and `--manual`
on `run` and `resume`.

### Why a harness is paused

A single line containing the word "quota" proves nothing — an agent may be
writing about quota code. Orion only pauses when **two independent signals**
agree:

1. **A pattern** in the last 40 lines of output, **and**
2. **A second signal**: the process exited non-zero, *or* it exited zero, printed
   fewer than 600 characters and changed no file according to `git status`.

Harnesses whose usage-limit message arrives with exit code 0 declare
`quota_ignores_exit_code` in the catalogue; for those, "short output" is enough.

Anything weaker opens a prompt instead of pausing. A reset time is taken from
the harness's own message (`resets in 2 hours`, `try again in 30 seconds`,
`resets at 14:30`) and otherwise a one-hour pause is applied.

### Safety rules

- **A paid harness only starts after a click.** Any tier other than `free` —
  including `unknown` — needs an explicit answer, in a terminal (pay / wait /
  stop, with a bell) or in a native dialog.
- **With no terminal and no desktop, Orion waits or stops. It never pays.**
- **The interface of an interactive tool is never automated.** Orion copies the
  prompt to the clipboard, tells you what to do, and waits for `next-harness resume`.
- **No telemetry.** Nothing is sent anywhere except what your harness already
  sends.
- **Secrets are masked** in logs and in `HANDOFF.md`.
- **One account per harness.** Orion never reads, copies or moves credentials.
- Pauses are persisted atomically: a crash cannot silently re-spend a quota.

### Configuration

- `profiles/builtin.toml` is the catalogue, compiled into the binary.
- Override it without recompiling: drop `*.toml` files in
  `<config_dir>/profiles.d/`. They are patches applied by harness `id`.
- `<config_dir>/config.json` holds `max_relays` and `manual`, written by
  `next-harness init` and shown by `next-harness config`.
- `<config_dir>/state.json` holds the pauses.

`<config_dir>` is the platform configuration directory (`%APPDATA%` on Windows,
`~/.config` on Linux, `~/Library/Application Support` on macOS).

### Compatibility

`confidence` says how much the catalogue entry can be trusted: `official`
(documentation from the vendor), `third party` (documented by a wrapper or the
community) or `unverified` (never tested on a real machine — please
[report what you see](.github/ISSUE_TEMPLATE/test-report.md)).

| Harness | id | Mode | Tier | Confidence |
| --- | --- | --- | --- | --- |
| OpenCode | `opencode` | headless | free | official |
| Kilo CLI | `kilo` | headless | free | official |
| Kilo Code CLI (older binary name) | `kilo-legacy` | headless | free | official |
| Crush (Charm) | `crush` | headless | unknown | official |
| Antigravity CLI | `agy` | headless | free | third party |
| Freebuff | `freebuff` | interactive | free | official |
| Cline CLI | `cline` | headless | unknown | official |
| Goose (Block) | `goose` | headless | unknown | third party |
| Aider | `aider` | headless | unknown | third party |
| Hermes Agent (Nous Research) | `hermes` | headless | unknown | official |
| Pi | `pi` | headless | unknown | unverified |
| Mistral Vibe | `vibe` | headless | unknown | official |
| Claude Code | `claude` | headless | paid | third party |
| Codex CLI | `codex` | headless | paid | third party |
| GitHub Copilot CLI | `copilot` | headless | unknown | third party |
| Cursor CLI | `cursor-agent` | headless | unknown | third party |
| Grok Build | `grok` | headless | paid | third party |
| Qwen Code | `qwen` | headless | paid | official |
| Droid (Factory) | `droid` | headless | paid | official |
| Gemini CLI (legacy) | `gemini` | headless | paid | third party |
| Whale (DeepSeek) | `whale` | headless | paid | official |
| Orca (DeepSeek) | `orca` | headless | paid | official |
| DeepSeek TUI | `deepseek-tui` | headless | paid | unverified |
| Antigravity (desktop app) | `antigravity-app` | gui | free | unverified |
| Freebuff Desktop | `freebuff-desktop` | gui | free | unverified |
| Cursor (editor) | `cursor-app` | gui | unknown | unverified |

A `gui` entry is only ever detected by name: Orion opens no desktop application
and drives no window.

### Honest limits

- **This beta has been exercised on Windows.** The Linux and macOS code paths
  (login-shell `PATH`, `.desktop` and `/Applications` scanning, GTK fallback)
  are written and reviewed but not yet run by the author; that is why several
  entries say `unverified`.
- **Windows process-tree killing uses `taskkill /T /F`**, not a job object: the
  dependency list for this version does not include a Windows API crate.
- **Times are shown in UTC.** Reset times printed by a harness in local time are
  read as UTC, so a pause can be a few hours too long or too short. It is
  recomputed on the next real attempt.
- **Detection is cached for 10 minutes** so `status` and `init` stay instant.
  `next-harness doctor --refresh` ignores the cache; installing a harness and running
  `next-harness run` immediately may need that refresh.
- **The catalogue is a snapshot.** Harnesses rename flags and binaries; when a
  probe fails the entry shows `not installed` rather than guessing.
- **WSL is detected, never driven.**
- **An agent without output is stopped after 120 s of silence** (plus the
  catalogue's `startup_grace_s`) and after 30 minutes in total. A stopped run is
  reported as *blocked*, never as *out of quota*.

### What Orion does not do

- It does not bypass quotas, rate limits, paywalls or logins.
- It does not run a cheaper model behind your back.
- It does not read, copy or sync credentials between tools.
- It does not drive a graphical application or a human-driven terminal session.
- It does not make your task succeed: it only makes sure the next free harness
  knows exactly where the last one stopped.

### Contributing

Bug reports, a harness entry, or a `doctor --report` from an untested machine
are all welcome. See [CONTRIBUTING.md](CONTRIBUTING.md).

### Support

Orion Relay is MIT and stays free. If it saved you a subscription:

- [GitHub Sponsors](https://github.com/sponsors/<OWNER>)
- [Ko-fi](https://ko-fi.com/<OWNER>)
- [Buy Me a Coffee](https://www.buymeacoffee.com/<OWNER>)

### License

[MIT](LICENSE).

---

## Français

### Pourquoi

Chaque agent de codage offre un quota gratuit, et chaque quota gratuit s'épuise
au milieu d'une tâche. Changer à la main, c'est relire le diff, retaper la
demande et espérer que l'outil suivant comprenne où le précédent s'est arrêté.

Orion Relay fait cette passation. Il lance la tâche avec le premier harnais de
votre liste, constate l'épuisement du quota, écrit ce qui a été fait, et démarre
le suivant sur la même réparation — avec le même fichier de tâche et le même
arbre de travail.

Ce n'est pas un agent. Il ne parle à aucun modèle, ne lit aucun jeton et ne
prétend jamais être l'un des outils qu'il pilote.

### Sommaire

- [Fonctionnement](#fonctionnement)
- [Installation](#installation)
- [Démarrage rapide](#démarrage-rapide)
- [Commandes](#commandes)
- [Pourquoi un harnais est mis en pause](#pourquoi-un-harnais-est-mis-en-pause)
- [Règles de sécurité](#règles-de-sécurité)
- [Compatibilité](#compatibilité)
- [Limites assumées](#limites-assumées)
- [Ce qu'Orion ne fait pas](#ce-qunext-harness-ne-fait-pas)

### Fonctionnement

```text
  vous ── next-harness run "ajoute un mode sombre" ──▶ .next-harness/TASK.md
                                                    │
                       ordre du catalogue           ▼
              ┌────────────────────▶ harnais #1 (gratuit) ──▶ terminé ──▶ fini
              │                          │
              │                          │ quota épuisé
              │                          ▼
              │            state.json : pause #1 jusqu'au reset (écriture atomique)
              │            .next-harness/HANDOFF.md : tâche, git status, diff --stat,
              │                              fin de sortie, « ne refais pas »
              │                          │
              └──────────────────────────┘  le suivant reçoit HANDOFF.md
                                                    │
                                        tous les candidats épuisés
                                                    ▼
                            gatekeeper : payer / attendre le reset / arrêter
```

La détection, la décision de quota et la passation sont locales. Le seul trafic
réseau est celui que votre harnais fait déjà.

### Installation

**Binaire prêt à l'emploi** — téléchargez l'archive de votre plateforme depuis
les [Releases](https://github.com/<OWNER>/next-harness-relay/releases), vérifiez le
`.sha256` à côté, puis placez `next-harness` dans votre `PATH`.

**Depuis les sources** — Rust 1.85 minimum :

```sh
cargo install --git https://github.com/<OWNER>/next-harness-relay --bin next-harness
```

Sous Linux, la compilation demande les en-têtes GTK et XDo (la question « payer »
est une boîte de dialogue native) :

```sh
sudo apt-get install libgtk-3-dev libxdo-dev
```

### Démarrage rapide

```sh
cd ~/code/mon-projet
next-harness init                          # détecte l'installé, écrit les réglages
next-harness doctor                        # ordre de relais et ce qui manque
next-harness run "ajoute un mode sombre et des tests"
```

C'est toute la boucle. Si le premier harnais épuise son quota, Orion le met en
pause, écrit `.next-harness/HANDOFF.md` et démarre le suivant. Quand tout est épuisé, il
demande : payer, attendre le reset, ou arrêter.

Chaque exécution laisse sa trace dans le dépôt :

```text
.next-harness/
├── TASK.md      la tâche, écrite une fois
├── HANDOFF.md   réécrit avant chaque bascule
└── runs/        transcriptions par tentative (ANSI retiré, secrets masqués)
```

### Commandes

| Commande | Effet |
| --- | --- |
| `next-harness init` | Détecte les harnais installés, affiche l'ordre de relais, écrit `config.json` |
| `next-harness run "<tâche>"` | Lance une tâche, avec relais sur quota |
| `next-harness resume` | Reprend la tâche déjà présente dans `.next-harness/TASK.md` |
| `next-harness status` | Quels harnais sont en pause, et jusqu'à quand |
| `next-harness doctor [--refresh] [--report]` | Inventaire ; `--report` produit du Markdown collable |
| `next-harness config` | Où vivent la configuration et l'état |

Options utiles : `--lang fr|en` sur toutes les commandes, `--max-relays <n>` et
`--manual` sur `run` et `resume`.

### Pourquoi un harnais est mis en pause

Une seule ligne contenant le mot « quota » ne prouve rien : un agent peut très
bien écrire du code sur les quotas. Orion ne met en pause que lorsque **deux
signaux indépendants** concordent :

1. **Un motif** dans les 40 dernières lignes de sortie, **et**
2. **Un second signal** : le processus a échoué (code ≠ 0), *ou* il a réussi
   mais a produit moins de 600 caractères et n'a modifié aucun fichier selon
   `git status`.

Les harnais dont le message de limite arrive avec le code 0 déclarent
`quota_ignores_exit_code` dans le catalogue ; pour eux, « sortie courte » suffit.

Tout signal plus faible ouvre une question au lieu d'une pause. L'heure de reset
est lue dans le message du harnais (`resets in 2 hours`, `try again in 30
seconds`, `resets at 14:30`), sinon une pause d'une heure s'applique.

### Règles de sécurité

- **Un harnais payant ne démarre qu'après un clic.** Toute catégorie autre que
  `free` — y compris `unknown` — exige une réponse explicite, dans un terminal
  (payer / attendre / arrêter, avec bip) ou dans une boîte de dialogue native.
- **Sans terminal ni interface, Orion attend ou s'arrête. Il ne paie jamais.**
- **L'interface d'un outil interactif n'est jamais automatisée.** Orion copie
  l'invite dans le presse-papiers, explique quoi faire et attend `next-harness resume`.
- **Aucune télémétrie.** Rien n'est envoyé, hormis ce que votre harnais envoie
  déjà.
- **Les secrets sont masqués** dans les journaux et dans `HANDOFF.md`.
- **Un compte par harnais.** Orion ne lit, ne copie et ne déplace aucun
  identifiant.
- Les pauses sont écrites de façon atomique : un crash ne peut pas dépenser un
  quota en silence.

### Compatibilité

`confidence` indique la confiance accordée à l'entrée du catalogue : `official`
(documentation de l'éditeur), `third party` (documenté par un wrapper ou la
communauté) ou `unverified` (jamais testé sur une vraie machine — merci de
[rapporter ce que vous voyez](.github/ISSUE_TEMPLATE/test-report.md)).

Le tableau complet, avec les identifiants et la colonne « confiance », se trouve
dans la [version anglaise](#compatibility). Une entrée `gui` n'est jamais
qu'un nom détecté : Orion n'ouvre aucune application de bureau et ne pilote
aucune fenêtre.

### Limites assumées

- **Cette bêta a été exercée sous Windows.** Les chemins Linux et macOS (`PATH`
  du shell de connexion, analyse `.desktop` et `/Applications`, repli GTK) sont
  écrits et relus, mais pas encore exécutés par l'auteur : c'est pourquoi
  plusieurs entrées affichent `unverified`.
- **Sous Windows, l'arbre de processus est tué par `taskkill /T /F`**, pas par
  un job object : la liste de dépendances de cette version n'inclut pas de crate
  d'API Windows.
- **Les heures sont affichées en UTC.** Une heure de reset annoncée en heure
  locale est lue comme UTC : la pause peut être trop longue ou trop courte de
  quelques heures. Elle est recalculée à la tentative réelle suivante.
- **La détection est mise en cache 10 minutes** pour que `status` et `init`
  restent instantanés. `next-harness doctor --refresh` ignore le cache ; installer un
  harnais puis lancer `next-harness run` aussitôt peut nécessiter ce refresh.
- **Le catalogue est un instantané.** Les harnais renomment options et binaires ;
  quand la sonde échoue, l'entrée indique `non installé` au lieu de deviner.
- **WSL est détecté, jamais piloté.**
- **Un agent silencieux est arrêté après 120 s sans sortie** (plus le
  `startup_grace_s` du catalogue) et après 30 minutes au total. Un arrêt de ce
  type est signalé comme *bloqué*, jamais comme *quota épuisé*.

### Ce qu'Orion ne fait pas

- Il ne contourne ni les quotas, ni les limites de débit, ni les paywalls, ni les
  connexions.
- Il ne lance aucun modèle moins cher dans votre dos.
- Il ne lit, ne copie et ne synchronise aucun identifiant entre outils.
- Il ne pilote aucune application graphique ni session de terminal humaine.
- Il ne fait pas réussir la tâche : il garantit seulement que le prochain harnais
  gratuit sait exactement où le précédent s'est arrêté.

### Contribuer

Rapports de bug, nouvelle entrée de harnais ou `doctor --report` d'une machine
non testée sont les bienvenus. Voir [CONTRIBUTING.md](CONTRIBUTING.md).

### Soutien

Orion Relay est sous MIT et reste gratuit. S'il vous a évité un abonnement :

- [GitHub Sponsors](https://github.com/sponsors/<OWNER>)
- [Ko-fi](https://ko-fi.com/<OWNER>)
- [Buy Me a Coffee](https://www.buymeacoffee.com/<OWNER>)

### Licence

[MIT](LICENSE).
