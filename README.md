<div align="center">

# NextHarness

**One coding task. Every AI coding harness you already have. No quota wasted.**

[![CI](https://github.com/SammyYohan/NextHarness/actions/workflows/ci.yml/badge.svg)](https://github.com/SammyYohan/NextHarness/actions/workflows/ci.yml)
[![release](https://img.shields.io/github/v/release/SammyYohan/NextHarness?include_prereleases)](https://github.com/SammyYohan/NextHarness/releases)
[![license](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org)

[English](#english) · [Français](#français) · [Español](#español) · [Deutsch](#deutsch) · [Italiano](#italiano) · [Português](#português) · [中文（简体）](#chinese-simplified) · [日本語](#japanese)

</div>

---

## English

### Why

Every coding agent gives you a free allowance, and every free allowance runs out
in the middle of a task. Switching by hand means re-reading the diff, retyping
the request, and hoping the next tool understands where the last one stopped.

NextHarness does that handoff. It starts your task with the first harness in
your list, notices when a harness is out of quota, writes down what was done, and
starts the next one on the same task — with the same task file and the same
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
- [Languages](#languages)
- [Compatibility](#compatibility)
- [Honest limits](#honest-limits)
- [What NextHarness does not do](#what-next-harness-does-not-do)
- [Contributing](#contributing)
- [Support](#support)

### How it works

```text
  you ── next-harness run "add a dark mode" ──▶ .next-harness/TASK.md
                                                │
                      catalog order             ▼
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

Detection, the quota decision and the handoff are all local. The only network
traffic is the one your harness already makes.

### Install

**One command** — download the latest release, verify its checksum, install it on
your `PATH`.

Windows (PowerShell):

```powershell
irm https://raw.githubusercontent.com/SammyYohan/NextHarness/main/install.ps1 | iex
```

macOS and Linux:

```sh
curl -fsSL https://raw.githubusercontent.com/SammyYohan/NextHarness/main/install.sh | sh
```

**Prebuilt binary** — download the archive for your platform from
[Releases](https://github.com/SammyYohan/NextHarness/releases), verify the
`.sha256` next to it, and put `next-harness` on your `PATH`.

**From source** — you need Rust 1.85 or newer:

```sh
cargo install --git https://github.com/SammyYohan/NextHarness --bin next-harness
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
next-harness doctor                        # see the handoff order and what is missing
next-harness run "add a dark mode and tests"
```

That is the whole loop. If the first harness runs out of quota, NextHarness pauses it,
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
|---|---|
| `next-harness init` | Detect installed harnesses, print the handoff order, write `config.json` |
| `next-harness run "<task>"` | Run a task, handing off on quota |
| `next-harness resume` | Continue the task already in `.next-harness/TASK.md` |
| `next-harness status` | Which harnesses are paused, and until when |
| `next-harness doctor [--refresh] [--report]` | Inventory; `--report` prints pasteable Markdown |
| `next-harness config` | Where the configuration and state live |

Useful flags: `--lang fr|en|es|de|it|pt|zh|ja` on any command, `--max-attempts <n>` and `--manual` on `run` and `resume`.

### Why a harness is paused

A single line containing the word "quota" proves nothing — an agent may be
writing about quota code. NextHarness only pauses when **two independent signals**
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
- **With no terminal and no desktop, NextHarness waits or stops. It never pays.**
- **The interface of an interactive tool is never automated.** NextHarness copies the
  prompt to the clipboard, tells you what to do, and waits for `next-harness resume`.
- **No telemetry.** Nothing is sent anywhere except what your harness already sends.
- **Secrets are masked** in logs and in `HANDOFF.md`.
- **One account per harness.** NextHarness never reads, copies or moves credentials.
- Pauses are persisted atomically: a crash cannot silently re-spend a quota.

### Configuration

- `profiles/builtin.toml` is the catalogue, compiled into the binary.
- Override it without recompiling: drop `*.toml` files in
  `<config_dir>/profiles.d/`. They are patches applied by harness `id`.
- `<config_dir>/config.json` holds `max_attempts` and `manual`, written by
  `next-harness init` and shown by `next-harness config`.
- `<config_dir>/state.json` holds the pauses.

`<config_dir>` is the platform configuration directory (`%APPDATA%` on Windows,
`~/.config` on Linux, `~/Library/Application Support` on macOS).

### Languages

NextHarness is translated into:

| Code | Language |
|---|---|
| `en` | English |
| `fr` | Français |
| `es` | Español |
| `de` | Deutsch |
| `it` | Italiano |
| `pt` | Português |
| `zh` | 中文（简体） |
| `ja` | 日本語 |

Pass `--lang <code>` to any command, or let NextHarness pick your system locale
automatically.

### Compatibility

`confidence` says how much the catalogue entry can be trusted: `official`
(documentation from the vendor), `third party` (documented by a wrapper or the
community) or `unverified` (never tested on a real machine — please
[report what you see](.github/ISSUE_TEMPLATE/test-report.md)).

| Harness | id | Mode | Tier | Confidence |
|---|---|---|---|---|
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

A `gui` entry is only ever detected by name: NextHarness opens no desktop application
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

### What NextHarness does not do

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

NextHarness is MIT and stays free. If it saved you a subscription:

- [GitHub Sponsors](https://github.com/sponsors/SammyYohan)
- [Ko-fi](https://ko-fi.com/SammyYohan)
- [Buy Me a Coffee](https://www.buymeacoffee.com/SammyYohan)

### License

[MIT](LICENSE).

---

## Français

### Pourquoi

Chaque agent de codage offre un quota gratuit, et chaque quota gratuit s'épuise
au milieu d'une tâche. Changer à la main, c'est relire le diff, retaper la
demande et espérer que l'outil suivant comprenne où le précédent s'est arrêté.

NextHarness fait cette passation. Il lance la tâche avec le premier harnais de
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
- [Configuration](#configuration)
- [Langues](#langues)
- [Compatibilité](#compatibilité)
- [Limites assumées](#limites-assumées)
- [Ce que NextHarness ne fait pas](#ce-que-nextharness-ne-fait-pas)
- [Contribuer](#contribuer)
- [Soutien](#soutien)

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

**Une commande** — télécharge la dernière version, vérifie son checksum SHA-256 et
l'installe dans votre `PATH`.

Windows (PowerShell) :

```powershell
irm https://raw.githubusercontent.com/SammyYohan/NextHarness/main/install.ps1 | iex
```

macOS et Linux :

```sh
curl -fsSL https://raw.githubusercontent.com/SammyYohan/NextHarness/main/install.sh | sh
```

**Binaire prêt à l'emploi** — téléchargez l'archive de votre plateforme depuis
les [Releases](https://github.com/SammyYohan/NextHarness/releases), vérifiez le
`.sha256` à côté, puis placez `next-harness` dans votre `PATH`.

**Depuis les sources** — Rust 1.85 minimum :

```sh
cargo install --git https://github.com/SammyYohan/NextHarness --bin next-harness
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
next-harness doctor                        # ordre de passation et ce qui manque
next-harness run "ajoute un mode sombre et des tests"
```

C'est toute la boucle. Si le premier harnais épuise son quota, NextHarness le met en
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
|---|---|
| `next-harness init` | Détecte les harnais installés, affiche l'ordre de passation, écrit `config.json` |
| `next-harness run "<tâche>"` | Lance une tâche, avec passation sur quota |
| `next-harness resume` | Reprend la tâche déjà présente dans `.next-harness/TASK.md` |
| `next-harness status` | Quels harnais sont en pause, et jusqu'à quand |
| `next-harness doctor [--refresh] [--report]` | Inventaire ; `--report` produit du Markdown collable |
| `next-harness config` | Où vivent la configuration et l'état |

Options utiles : `--lang fr|en|es|de|it|pt|zh|ja` sur toutes les commandes, `--max-attempts <n>` et
`--manual` sur `run` et `resume`.

### Pourquoi un harnais est mis en pause

Une seule ligne contenant le mot « quota » ne prouve rien : un agent peut très
bien écrire du code sur les quotas. NextHarness ne met en pause que lorsque **deux
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
- **Sans terminal ni interface, NextHarness attend ou s'arrête. Il ne paie jamais.**
- **L'interface d'un outil interactif n'est jamais automatisée.** NextHarness copie
  l'invite dans le presse-papiers, explique quoi faire et attend `next-harness resume`.
- **Aucune télémétrie.** Rien n'est envoyé, hormis ce que votre harnais envoie
  déjà.
- **Les secrets sont masqués** dans les journaux et dans `HANDOFF.md`.
- **Un compte par harnais.** NextHarness ne lit, ne copie et ne déplace aucun
  identifiant.
- Les pauses sont écrites de façon atomique : un crash ne peut pas dépenser un
  quota en silence.

### Configuration

- `profiles/builtin.toml` est le catalogue, compilé dans le binaire.
- Remplacez-le sans recompiler : déposez des fichiers `*.toml` dans
  `<config_dir>/profiles.d/`. Ce sont des correctifs appliqués par `id` de harnais.
- `<config_dir>/config.json` contient `max_attempts` et `manual`, écrits par
  `next-harness init` et affichés par `next-harness config`.
- `<config_dir>/state.json` contient les pauses.

`<config_dir>` est le répertoire de configuration de la plateforme
(`%APPDATA%` sous Windows, `~/.config` sous Linux, `~/Library/Application Support`
sous macOS).

### Langues

NextHarness est traduit en :

| Code | Langue |
|---|---|
| `en` | Anglais |
| `fr` | Français |
| `es` | Espagnol |
| `de` | Allemand |
| `it` | Italien |
| `pt` | Portugais |
| `zh` | Chinois simplifié |
| `ja` | Japonais |

Passez `--lang <code>` à n'importe quelle commande, ou laissez NextHarness choisir
automatiquement la langue de votre système.

### Compatibilité

`confidence` indique la confiance accordée à l'entrée du catalogue : `official`
(documentation de l'éditeur), `third party` (documenté par un wrapper ou la
communauté) ou `unverified` (jamais testé sur une vraie machine — merci de
[rapporter ce que vous voyez](.github/ISSUE_TEMPLATE/test-report.md)).

Une entrée `gui` n'est jamais qu'un nom détecté : NextHarness n'ouvre aucune
application de bureau et ne pilote aucune fenêtre.

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

### Ce que NextHarness ne fait pas

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

NextHarness est sous MIT et reste gratuit. S'il vous a évité un abonnement :

- [GitHub Sponsors](https://github.com/sponsors/SammyYohan)
- [Ko-fi](https://ko-fi.com/SammyYohan)
- [Buy Me a Coffee](https://www.buymeacoffee.com/SammyYohan)

### Licence

[MIT](LICENSE).

---

## Español

### Por qué

Cada agente de programación te da una cuota gratuita, y cada cuota gratuita se agota
en medio de una tarea. Cambiar manualmente implica volver a leer el diff, volver a escribir
la solicitud y esperar que la siguiente herramienta entienda dónde se quedó la anterior.

NextHarness hace ese traspaso. Inicia tu tarea con el primer harness de tu
lista, se da cuenta cuando un harness se queda sin cuota, anota lo que se hizo e
inicia el siguiente en la misma tarea — con el mismo archivo de tarea y el mismo
árbol de trabajo.

No es un agente. Nunca habla con un modelo, nunca lee un token, y nunca
finge ser una de las herramientas que maneja.

### Contenido

- [Cómo funciona](#cómo-funciona)
- [Instalación](#instalación)
- [Inicio rápido](#inicio-rápido)
- [Comandos](#comandos)
- [Por qué se pausa un harness](#por-qué-se-pausa-un-harness)
- [Reglas de seguridad](#reglas-de-seguridad)
- [Configuración](#configuración)
- [Idiomas](#idiomas)
- [Compatibilidad](#compatibilidad)
- [Límites honestos](#límites-honestos)
- [Lo que NextHarness no hace](#lo-que-nextharness-no-hace)
- [Contribuir](#contribuir)
- [Soporte](#soporte)

### Cómo funciona

```text
  tú ── next-harness run "añadir modo oscuro" ──▶ .next-harness/TASK.md
                                                  │
                    orden del catálogo            ▼
        ┌──────────────────▶ harness #1 (gratis) ──▶ terminado ──▶ listo
        │                       │
        │                       │ sin cuota
        │                       ▼
        │            state.json: pausa #1 hasta el reinicio (escritura atómica)
        │            .next-harness/HANDOFF.md: tarea, git status, diff --stat,
        │                              fin de salida, "no rehacer"
        │                       │
        └───────────────────────┘  el siguiente harness recibe HANDOFF.md
                                                  │
                                          todos los candidatos agotados
                                                  ▼
                              gatekeeper: pagar ahora / esperar reinicio / detener
```

La detección, la decisión de cuota y el traspaso son todos locales. El único tráfico
de red es el que ya hace tu harness.

### Instalación

**Un comando** — descarga la última versión, verifica su checksum SHA-256 y lo
instala en tu `PATH`.

Windows (PowerShell):

```powershell
irm https://raw.githubusercontent.com/SammyYohan/NextHarness/main/install.ps1 | iex
```

macOS y Linux:

```sh
curl -fsSL https://raw.githubusercontent.com/SammyYohan/NextHarness/main/install.sh | sh
```

**Binario precompilado** — descarga el archivo para tu plataforma desde
[Releases](https://github.com/SammyYohan/NextHarness/releases), verifica el
`.sha256` junto a él, y coloca `next-harness` en tu `PATH`.

**Desde el código fuente** — necesitas Rust 1.85 o superior:

```sh
cargo install --git https://github.com/SammyYohan/NextHarness --bin next-harness
```

En Linux la compilación necesita los encabezados GTK y XDo, porque el aviso de pago es un
diálogo nativo:

```sh
sudo apt-get install libgtk-3-dev libxdo-dev
```

### Inicio rápido

```sh
cd ~/code/mi-proyecto
next-harness init                          # detecta lo instalado, escribe valores por defecto
next-harness doctor                        # ve el orden de traspaso y qué falta
next-harness run "añadir modo oscuro y pruebas"
```

Ese es todo el ciclo. Si el primer harness se queda sin cuota, NextHarness lo pausa,
escribe `.next-harness/HANDOFF.md`, e inicia el siguiente. Cuando todo está
agotado pregunta: pagar, esperar el reinicio, o detener.

Cada ejecución deja su rastro en el repositorio:

```text
.next-harness/
├── TASK.md      la tarea, escrita una vez
├── HANDOFF.md   reescrito antes de cada cambio
└── runs/        transcripciones por intento (ANSI eliminado, secretos enmascarados)
```

### Comandos

| Comando | Qué hace |
|---|---|
| `next-harness init` | Detecta harnesses instalados, imprime el orden de traspaso, escribe `config.json` |
| `next-harness run "<tarea>"` | Ejecuta una tarea, relevando por cuota |
| `next-harness resume` | Continúa la tarea ya en `.next-harness/TASK.md` |
| `next-harness status` | Qué harnesses están pausados, y hasta cuándo |
| `next-harness doctor [--refresh] [--report]` | Inventario; `--report` imprime Markdown pegable |
| `next-harness config` | Dónde viven la configuración y el estado |

Banderas útiles: `--lang es|en|fr|de|it|pt|zh|ja` en cualquier comando, `--max-attempts <n>` y `--manual` en `run` y `resume`.

### Por qué se pausa un harness

Una sola línea que contiene la palabra "cuota" no prueba nada — un agente puede estar
escribiendo sobre código de cuota. NextHarness solo pausa cuando **dos señales
independientes** coinciden:

1. **Un patrón** en las últimas 40 líneas de salida, **y**
2. **Una segunda señal**: el proceso salió con código no cero, *o* salió con cero, imprimió
   menos de 600 caracteres y no cambió ningún archivo según `git status`.

Los harnesses cuyo mensaje de límite de uso llega con código de salida 0 declaran
`quota_ignores_exit_code` en el catálogo; para esos, "salida corta" es suficiente.

Cualquier cosa más débil abre una pregunta en lugar de pausar. Un tiempo de reinicio se toma
del mensaje del propio harness (`resets in 2 hours`, `try again in 30 seconds`,
`resets at 14:30`) y si no, se aplica una pausa de una hora.

### Reglas de seguridad

- **Un harness de pago solo inicia después de un clic.** Cualquier nivel distinto de `gratis`
  — incluyendo `desconocido` — necesita una respuesta explícita, en una terminal (pagar /
  esperar / cancelar, con un timbre) o en un diálogo nativo.
- **Sin terminal y sin escritorio, NextHarness espera o se detiene. Nunca paga.**
- **La interfaz de una herramienta interactiva nunca se automatiza.** NextHarness copia el
  mensaje al portapapeles, te dice qué hacer, y espera `next-harness resume`.
- **Sin telemetría.** No se envía nada a ningún lado excepto lo que tu harness ya envía.
- **Los secretos se enmascaran** en los registros y en `HANDOFF.md`.
- **Una cuenta por harness.** NextHarness nunca lee, copia ni mueve credenciales.
- Las pausas se persisten de forma atómica: un fallo no puede volver a gastar una cuota en silencio.

### Configuración

- `profiles/builtin.toml` es el catálogo, compilado en el binario.
- Sobrescríbalo sin recompilar: coloca archivos `*.toml` en
  `<config_dir>/profiles.d/`. Son parches aplicados por `id` del harness.
- `<config_dir>/config.json` almacena `max_attempts` y `manual`, escritos por
  `next-harness init` y mostrados por `next-harness config`.
- `<config_dir>/state.json` almacena las pausas.

`<config_dir>` es el directorio de configuración de la plataforma (`%APPDATA%` en Windows,
`~/.config` en Linux, `~/Library/Application Support` en macOS).

### Idiomas

NextHarness está traducido a:

| Código | Idioma |
|---|---|
| `en` | Inglés |
| `fr` | Francés |
| `es` | Español |
| `de` | Alemán |
| `it` | Italiano |
| `pt` | Portugués |
| `zh` | Chino simplificado |
| `ja` | Japonés |

Pasa `--lang <código>` a cualquier comando, o deja que NextHarness elija automáticamente
el idioma de tu sistema.

### Compatibilidad

`confidence` indica cuánto se puede confiar en una entrada del catálogo: `official`
(documentación del proveedor), `third party` (documentada por un envoltorio o la
comunidad) o `unverified` (nunca probado en una máquina real — por favor
[reporta lo que ves](.github/ISSUE_TEMPLATE/test-report.md)).

Una entrada `gui` solo se detecta por nombre: NextHarness no abre ninguna aplicación de escritorio
y no maneja ninguna ventana.

### Límites honestos

- **Esta beta se ha probado en Windows.** Las rutas de código de Linux y macOS
  (`PATH` del shell de inicio, escaneo de `.desktop` y `/Applications`, respaldo GTK)
  están escritas y revisadas pero aún no ejecutadas por el autor; por eso varias
  entradas dicen `unverified`.
- **En Windows, el árbol de procesos se mata con `taskkill /T /F`**, no con un objeto de trabajo: la
  lista de dependencias de esta versión no incluye un crate de API de Windows.
- **Los tiempos se muestran en UTC.** Los tiempos de reinicio impresos por un harness en hora local se
  leen como UTC, por lo que una pausa puede ser unas pocas horas demasiado larga o corta. Se
  recalcula en el siguiente intento real.
- **La detección está en caché por 10 minutos** para que `status` y `init` se mantengan instantáneos.
  `next-harness doctor --refresh` ignora la caché; instalar un harness y ejecutar
  `next-harness run` inmediatamente puede necesitar ese refresco.
- **El catálogo es una instantánea.** Los harnesses renombran banderas y binarios; cuando una
  sonda falla, la entrada muestra `no instalado` en lugar de adivinar.
- **WSL se detecta, nunca se maneja.**
- **Un agente sin salida se detiene después de 120 s de silencio** (más el
  `startup_grace_s` del catálogo) y después de 30 minutos en total. Una ejecución detenida se
  reporta como *bloqueada*, nunca como *sin cuota*.

### Lo que NextHarness no hace

- No evita cuotas, límites de velocidad, paywalls o inicios de sesión.
- No ejecuta un modelo más barato a tus espaldas.
- No lee, copia ni sincroniza credenciales entre herramientas.
- No maneja una aplicación gráfica ni una sesión de terminal dirigida por un humano.
- No hace que tu tarea tenga éxito: solo asegura que el siguiente harness gratuito sepa exactamente
  dónde se quedó el anterior.

### Contribuir

Informes de errores, una entrada de harness, o un `doctor --report` de una máquina no probada
son bienvenidos. Consulta [CONTRIBUTING.md](CONTRIBUTING.md).

### Soporte

NextHarness es MIT y sigue siendo gratuito. Si te ahorró una suscripción:

- [GitHub Sponsors](https://github.com/sponsors/SammyYohan)
- [Ko-fi](https://ko-fi.com/SammyYohan)
- [Buy Me a Coffee](https://www.buymeacoffee.com/SammyYohan)

### Licencia

[MIT](LICENSE).

---

## Deutsch

### Warum

Jeder Coding-Agent gibt Ihnen ein kostenloses Kontingent, und jedes kostenlose Kontingent läuft
inmitten einer Aufgabe ab. Der manuelle Wechsel bedeutet, den Diff erneut zu lesen, die
Anfrage neu einzutippen und zu hoffen, dass das nächste Tool versteht, wo das letzte aufgehört hat.

NextHarness übernimmt diese Übergabe. Es startet Ihre Aufgabe mit dem ersten Harness in
Ihrer Liste, bemerkt, wenn ein Harness ohne Kontingent ist, notiert, was getan wurde, und
startet das nächste bei derselben Aufgabe — mit derselben Aufgaben-Datei und demselben
Arbeitsbaum.

Es ist kein Agent. Es spricht niemals mit einem Modell, liest niemals einen Token und gibt
sich niemals als eines der Tools aus, die es steuert.

### Inhaltsverzeichnis

- [Wie es funktioniert](#wie-es-funktioniert)
- [Installation](#installation)
- [Schnellstart](#schnellstart)
- [Befehle](#befehle)
- [Warum ein Harness pausiert wird](#warum-ein-harness-pausiert-wird)
- [Sicherheitsregeln](#sicherheitsregeln)
- [Konfiguration](#konfiguration)
- [Sprachen](#sprachen)
- [Kompatibilität](#kompatibilität)
- [Ehrliche Grenzen](#ehrliche-grenzen)
- [Was NextHarness nicht tut](#was-nextharness-nicht-tut)
- [Mitwirken](#mitwirken)
- [Support](#support)

### Wie es funktioniert

```text
  Sie ── next-harness run "dunklen Modus hinzufügen" ──▶ .next-harness/TASK.md
                                                        │
                       Katalogreihenfolge               ▼
        ┌──────────────────▶ Harness #1 (kostenlos) ──▶ fertig ──▶ erledigt
        │                       │
        │                       │ ohne Kontingent
        │                       ▼
        │            state.json: #1 bis zum Zurücksetzen pausieren (atomarer Schreibvorgang)
        │            .next-harness/HANDOFF.md: Aufgabe, git status, diff --stat,
        │                              Ende der Ausgabe, "nicht wiederholen"
        │                       │
        └───────────────────────┘  nächstes Harness erhält HANDOFF.md
                                                        │
                                              alle Kandidaten erschöpft
                                                        ▼
                              Gatekeeper: jetzt bezahlen / auf Zurücksetzen warten / stoppen
```

Die Erkennung, die Kontingententscheidung und die Übergabe sind allesamt lokal. Der einzige
Netzwerkverkehr ist der, den Ihr Harness bereits erzeugt.

### Installation

**Ein Befehl** — lädt die neueste Version herunter, prüft ihre SHA-256-Prüfsumme
und installiert sie auf Ihren `PATH`.

Windows (PowerShell):

```powershell
irm https://raw.githubusercontent.com/SammyYohan/NextHarness/main/install.ps1 | iex
```

macOS und Linux:

```sh
curl -fsSL https://raw.githubusercontent.com/SammyYohan/NextHarness/main/install.sh | sh
```

**Vorgekompilierte Binärdatei** — laden Sie das Archiv für Ihre Plattform von
[Releases](https://github.com/SammyYohan/NextHarness/releases) herunter, verifizieren Sie die
`.sha256` daneben, und legen Sie `next-harness` auf Ihren `PATH`.

**Aus dem Quellcode** — Sie benötigen Rust 1.85 oder neuer:

```sh
cargo install --git https://github.com/SammyYohan/NextHarness --bin next-harness
```

Unter Linux werden die GTK- und XDo-Header benötigt, weil die Zahlungsaufforderung ein
nativer Dialog ist:

```sh
sudo apt-get install libgtk-3-dev libxdo-dev
```

### Schnellstart

```sh
cd ~/code/mein-projekt
next-harness init                          # erkennt Installationen, schreibt Standardeinstellungen
next-harness doctor                        # zeigt die Weiterleitungsreihenfolge und was fehlt
next-harness run "dunklen Modus und Tests hinzufügen"
```

Das ist der gesamte Kreislauf. Wenn das erste Harness ohne Kontingent ist, pausiert NextHarness es,
schreibt `.next-harness/HANDOFF.md` und startet das nächste. Wenn alles erschöpft ist, fragt es:
bezahlen, auf den Reset warten oder stoppen.

Jeder Lauf hinterlässt seine Spuren im Repository:

```text
.next-harness/
├── TASK.md      die Aufgabe, einmal geschrieben
├── HANDOFF.md   vor jedem Wechsel neu geschrieben
└── runs/        Pro-Versuch-Protokolle (ANSI entfernt, Geheimnisse maskiert)
```

### Befehle

| Befehl | Was er tut |
|---|---|
| `next-harness init` | Installierte Harnesses erkennen, Weiterleitungsreihenfolge ausgeben, `config.json` schreiben |
| `next-harness run "<aufgabe>"` | Eine Aufgabe ausführen, mit Weiterleitung bei Kontingent |
| `next-harness resume` | Die bereits in `.next-harness/TASK.md` gespeicherte Aufgabe fortsetzen |
| `next-harness status` | Welche Harnesses pausiert sind und bis wann |
| `next-harness doctor [--refresh] [--report]` | Inventur; `--report` gibt kopierbares Markdown aus |
| `next-harness config` | Wo Konfiguration und Zustand gespeichert sind |

Nützliche Flags: `--lang de|en|fr|es|it|pt|zh|ja` bei jedem Befehl, `--max-attempts <n>` und `--manual` bei `run` und `resume`.

### Warum ein Harness pausiert wird

Eine einzelne Zeile mit dem Wort "Kontingent" beweist nichts — ein Agent kann über
Kontingent-Code schreiben. NextHarness pausiert nur, wenn **zwei unabhängige Signale**
übereinstimmen:

1. **Ein Muster** in den letzten 40 Zeilen der Ausgabe, **und**
2. **Ein zweites Signal**: der Prozess ist mit einem nicht-null-Exitcode beendet, *oder* er ist
   mit null beendet, hat weniger als 600 Zeichen ausgegeben und keine Datei gemäß `git status`
   geändert.

Harnesses, deren Nutzungslimit-Nachricht mit Exitcode 0 eintrifft, deklarieren
`quota_ignores_exit_code` im Katalog; für diese reicht "kurze Ausgabe".

Alles Schwächeres öffnet eine Frage anstelle einer Pause. Eine Zurücksetzungszeit wird aus
der eigenen Nachricht des Harnesses übernommen (`resets in 2 hours`, `try again in 30 seconds`,
`resets at 14:30`) und sonst wird eine einstündige Pause angewendet.

### Sicherheitsregeln

- **Ein kostenpflichtiges Harness startet erst nach einem Klick.** Jede Stufe außer `kostenlos`
  — einschließlich `unbekannt` — benötigt eine explizite Antwort, in einem Terminal (bezahlen /
  warten / abbrechen, mit einem Signalton) oder in einem nativen Dialog.
- **Ohne Terminal und ohne Desktop wartet oder stoppt NextHarness. Es bezahlt niemals.**
- **Die Oberfläche eines interaktiven Tools wird niemals automatisiert.** NextHarness kopiert die
  Aufforderung in die Zwischenablage, sagt Ihnen, was zu tun ist, und wartet auf `next-harness resume`.
- **Keine Telemetrie.** Nichts wird irgendwo gesendet, außer was Ihr Harness bereits sendet.
- **Geheimnisse werden maskiert** in Protokollen und in `HANDOFF.md`.
- **Ein Konto pro Harness.** NextHarness liest, kopiert oder verschiebt niemals Anmeldeinformationen.
- Pausen werden atomar gespeichert: ein Absturz kann nicht stillschweigend ein Kontingent erneut ausgeben.

### Konfiguration

- `profiles/builtin.toml` ist der Katalog, kompiliert in die Binärdatei.
- Überschreiben Sie ihn ohne Neukompilierung: legen Sie `*.toml`-Dateien in
  `<config_dir>/profiles.d/`. Sie sind Patches, die nach Harness-`id` angewendet werden.
- `<config_dir>/config.json` enthält `max_attempts` und `manual`, geschrieben von
  `next-harness init` und angezeigt von `next-harness config`.
- `<config_dir>/state.json` enthält die Pausen.

`<config_dir>` ist das platformabhängige Konfigurationsverzeichnis (`%APPDATA%` unter Windows,
`~/.config` unter Linux, `~/Library/Application Support` unter macOS).

### Sprachen

NextHarness ist übersetzt in:

| Code | Sprache |
|---|---|
| `en` | Englisch |
| `fr` | Französisch |
| `es` | Spanisch |
| `de` | Deutsch |
| `it` | Italienisch |
| `pt` | Portugiesisch |
| `zh` | Chinesisch (vereinfacht) |
| `ja` | Japanisch |

Übergeben Sie `--lang <code>` an jeden Befehl, oder lassen Sie NextHarness automatisch Ihre
Systemsprache wählen.

### Kompatibilität

`confidence` sagt, wie sehr ein Katalogeintrag vertrauenswürdig ist: `official`
(Dokumentation des Anbieters), `third party` (dokumentiert durch einen Wrapper oder die
Community) oder `unverified` (noch nie auf einer echten Maschine getestet — bitte
[berichten Sie, was Sie sehen](.github/ISSUE_TEMPLATE/test-report.md)).

Ein `gui`-Eintrag wird nur nach Name erkannt: NextHarness öffnet keine Desktop-Anwendung
und steuert kein Fenster.

### Ehrliche Grenzen

- **Diese Beta wurde unter Windows erprobt.** Die Linux- und macOS-Codepfade
  (Login-Shell-`PATH`, `.desktop`- und `/Applications`-Scannen, GTK-Fallback)
  sind geschrieben und überprüft, aber noch nicht vom Autor ausgeführt; daher sind mehrere
  Einträge als `unverified` gekennzeichnet.
- **Unter Windows wird der Prozessbaum mit `taskkill /T /F` beendet**, nicht mit einem Job-Objekt: die
  Abhängigkeitsliste dieser Version enthält kein Windows-API-Crate.
- **Zeiten werden in UTC angezeigt.** Zurücksetzungszeiten, die ein Harness in Ortszeit ausgibt, werden
  als UTC gelesen, sodass eine Pause ein paar Stunden zu lang oder zu kurz sein kann. Sie wird
  beim nächsten echten Versuch neu berechnet.
- **Die Erkennung ist 10 Minuten gecacht**, damit `status` und `init` sofort reagieren.
  `next-harness doctor --refresh` ignoriert den Cache; die Installation eines Harnesses und das sofortige
  Ausführen von `next-harness run` kann diesen Refresh benötigen.
- **Der Katalog ist ein Snapshot.** Harnesses benennen Flags und Binärdateien um; wenn eine
  Prüfung fehlschlägt, zeigt der Eintrag `nicht installiert` anstatt zu raten.
- **WSL wird erkannt, nie gesteuert.**
- **Ein Harness ohne Ausgabe wird nach 120 s Stille gestoppt** (plus dem `startup_grace_s` des
  Katalogs) und nach insgesamt 30 Minuten. Ein gestopperter Lauf wird als *blockiert* gemeldet,
  niemals als *ohne Kontingent*.

### Was NextHarness nicht tut

- Es umgeht keine Kontingente, Ratenlimits, Bezahlschranken oder Logins.
- Es führt kein günstigeres Modell hinter Ihrem Rücken aus.
- Es liest, kopiert oder synchronisiert keine Anmeldeinformationen zwischen Tools.
- Es steuert keine grafische Anwendung oder keine von einem Menschen gesteuerte Terminalsitzung.
- Es lässt Ihre Aufgabe nicht gelingen: Es stellt nur sicher, dass das nächste kostenlose Harness
  genau weiß, wo das letzte aufgehört hat.

### Mitwirken

Fehlerberichte, ein Harness-Eintrag oder ein `doctor --report` von einer ungetesteten Maschine
sind willkommen. Siehe [CONTRIBUTING.md](CONTRIBUTING.md).

### Support

NextHarness ist MIT und bleibt kostenlos. Wenn es Ihnen ein Abo erspart hat:

- [GitHub Sponsors](https://github.com/sponsors/SammyYohan)
- [Ko-fi](https://ko-fi.com/SammyYohan)
- [Buy Me a Coffee](https://www.buymeacoffee.com/SammyYohan)

### Lizenz

[MIT](LICENSE).

---

## Italiano

### Perché

Ogni agente di programmazione ti concede una quota gratuita, e ogni quota gratuita si esaurisce
a metà di un compito. Cambiare manualmente significa rileggere il diff, riscrivere
la richiesta e sperare che il prossimo strumento capisca dove si è fermato il precedente.

NextHarness gestisce questo passaggio. Inizia il tuo compito con il primo harness nella
tua lista, si accorge quando un harness è senza quota, registra cosa è stato fatto e
avvia il successivo nello stesso compito — con lo stesso file di compito e lo stesso
albero di lavoro.

Non è un agente. Non parla mai con un modello, non legge mai un token e non
finge di essere uno degli strumenti che guida.

### Indice

- [Come funziona](#come-funziona)
- [Installazione](#installazione)
- [Avvio rapido](#avvio-rapido)
- [Comandi](#comandi)
- [Perché un harness viene messo in pausa](#perché-un-harness-viene-messo-in-pausa)
- [Regole di sicurezza](#regole-di-sicurezza)
- [Configurazione](#configurazione)
- [Lingue](#lingue)
- [Compatibilità](#compatibilità)
- [Limiti onesti](#limiti-onesti)
- [Cosa NextHarness non fa](#cosa-nextharness-non-fa)
- [Contribuire](#contribuire)
- [Supporto](#supporto)

### Come funziona

```text
  tu ── next-harness run "aggiungi modalità scura" ──▶ .next-harness/TASK.md
                                                       │
                  ordine del catalogo                   ▼
        ┌──────────────────▶ harness #1 (gratuito) ──▶ finito ──▶ completato
        │                       │
        │                       │ senza quota
        │                       ▼
        │            state.json: pausa #1 fino al reset (scrittura atomica)
        │            .next-harness/HANDOFF.md: compito, git status, diff --stat,
        │                              fine output, "non ripetere"
        │                       │
        └───────────────────────┘  il prossimo harness riceve HANDOFF.md
                                                       │
                                             ogni candidato esaurito
                                                       ▼
                              gatekeeper: paga ora / aspetta il reset / ferma
```

La rilevazione, la decisione sulla quota e il passaggio sono tutti locali. L'unico traffico
di rete è quello già effettuato dal tuo harness.

### Installazione

**Un comando** — scarica l'ultima versione, ne verifica il checksum SHA-256 e la
installa nel tuo `PATH`.

Windows (PowerShell):

```powershell
irm https://raw.githubusercontent.com/SammyYohan/NextHarness/main/install.ps1 | iex
```

macOS e Linux:

```sh
curl -fsSL https://raw.githubusercontent.com/SammyYohan/NextHarness/main/install.sh | sh
```

**Binario precompilato** — scarica l'archivio per la tua piattaforma da
[Releases](https://github.com/SammyYohan/NextHarness/releases), verifica il
`.sha256` accanto e metti `next-harness` nel tuo `PATH`.

**Da sorgente** — serve Rust 1.85 o più recente:

```sh
cargo install --git https://github.com/SammyYohan/NextHarness --bin next-harness
```

Su Linux la compilazione richiede le intestazioni GTK e XDo, perché il prompt di pagamento è una
finestra di dialogo nativa:

```sh
sudo apt-get install libgtk-3-dev libxdo-dev
```

### Avvio rapido

```sh
cd ~/code/mio-progetto
next-harness init                          # rileva ciò che è installato, scrive i valori predefiniti
next-harness doctor                        # vedi l'ordine di passaggio e cosa manca
next-harness run "aggiungi modalità scura e test"
```

Questo è l'intero ciclo. Se il primo harness finisce la quota, NextHarness lo mette in pausa,
scrive `.next-harness/HANDOFF.md` e avvia il successivo. Quando tutto è
esaurito chiede: paga, aspetta il reset o ferma.

Ogni esecuzione lascia le sue tracce nel repository:

```text
.next-harness/
├── TASK.md      il compito, scritto una volta
├── HANDOFF.md   riscritto prima di ogni cambio
└── runs/        trascrizioni per tentativo (ANSI rimosso, segreti mascherati)
```

### Comandi

| Comando | Cosa fa |
|---|---|
| `next-harness init` | Rileva harness installati, stampa l'ordine di passaggio, scrive `config.json` |
| `next-harness run "<compito>"` | Esegue un compito, con passaggio su quota |
| `next-harness resume` | Continua il compito già in `.next-harness/TASK.md` |
| `next-harness status` | Quali harness sono in pausa e fino a quando |
| `next-harness doctor [--refresh] [--report]` | Inventario; `--report` stampa Markdown incollabile |
| `next-harness config` | Dove vivono configurazione e stato |

Flag utili: `--lang it|en|fr|es|de|pt|zh|ja` su qualsiasi comando, `--max-attempts <n>` e `--manual` su `run` e `resume`.

### Perché un harness viene messo in pausa

Una singola riga contenente la parola "quota" non prova nulla — un agente potrebbe
scrivere codice sulla quota. NextHarness mette in pausa solo quando **due segnali
indipendenti** concordano:

1. **Un pattern** nelle ultime 40 righe di output, **e**
2. **Un secondo segnale**: il processo è uscito con codice non zero, *o* è uscito
   con zero, ha stampato meno di 600 caratteri e non ha modificato file secondo `git status`.

Gli harness il cui messaggio di limite d'uso arriva con codice di uscita 0 dichiarano
`quota_ignores_exit_code` nel catalogo; per questi, "output breve" basta.

Tutto ciò che è più debole apre un prompt invece di mettere in pausa. Il tempo di reset viene
preso dal messaggio dell'harness stesso (`resets in 2 hours`, `try again in 30 seconds`,
`resets at 14:30`) altrimenti si applica una pausa di un'ora.

### Regole di sicurezza

- **Un harness a pagamento parte solo dopo un clic.** Qualsiasi livello diverso da `gratuito`
  — incluso `sconosciuto` — richiede una risposta esplicita, in un terminale (paga / aspetta /
  ferma, con un suono) o in una finestra di dialogo nativa.
- **Senza terminale e senza desktop, NextHarness aspetta o si ferma. Non paga mai.**
- **L'interfaccia di uno strumento interattivo non viene mai automatizzata.** NextHarness copia il
  prompt negli appunti, ti dice cosa fare e aspetta `next-harness resume`.
- **Nessuna telemetria.** Niente viene inviato se non ciò che il tuo harness invia già.
- **I segreti sono mascherati** nei log e in `HANDOFF.md`.
- **Un account per harness.** NextHarness non legge, copia né sposta mai le credenziali.
- Le pause sono persistite in modo atomico: un crash non può silenziosamente riespendere una quota.

### Configurazione

- `profiles/builtin.toml` è il catalogo, compilato nel binario.
- Sovrascrivilo senza ricompilare: inserisci file `*.toml` in
  `<config_dir>/profiles.d/`. Sono patch applicati per `id` dell'harness.
- `<config_dir>/config.json` contiene `max_attempts` e `manual`, scritti da
  `next-harness init` e mostrati da `next-harness config`.
- `<config_dir>/state.json` contiene le pause.

`<config_dir>` è la directory di configurazione della piattaforma (`%APPDATA%` su Windows,
`~/.config` su Linux, `~/Library/Application Support` su macOS).

### Lingue

NextHarness è tradotto in:

| Codice | Lingua |
|---|---|
| `en` | Inglese |
| `fr` | Francese |
| `es` | Spagnolo |
| `de` | Tedesco |
| `it` | Italiano |
| `pt` | Portoghese |
| `zh` | Cinese semplificato |
| `ja` | Giapponese |

Passa `--lang <codice>` a qualsiasi comando, o lascia che NextHarness scelga automaticamente
la lingua del tuo sistema.

### Compatibilità

`confidence` indica quanto ci si può fidare di un'entrata del catalogo: `official`
(documentazione del fornitore), `third party` (documentata da un wrapper o dalla
comunità) o `unverified` (mai testato su una macchina reale — per favore
[segnala cosa vedi](.github/ISSUE_TEMPLATE/test-report.md)).

Un'entrata `gui` viene rilevata solo per nome: NextHarness non apre alcuna applicazione
desktop e non guida alcuna finestra.

### Limiti onesti

- **Questa beta è stata esercitata su Windows.** I percorsi di codice Linux e macOS
  (`PATH` della shell di login, scansione `.desktop` e `/Applications`, fallback GTK)
  sono scritti e revisionati ma non ancora eseguiti dall'autore; per questo diverse
  voci dicono `unverified`.
- **Su Windows l'albero dei processi viene ucciso con `taskkill /T /F`**, non con un job object: la
  lista delle dipendenze di questa versione non include una crate API di Windows.
- **I tempi sono mostrati in UTC.** I tempi di reset stampati da un harness in orario locale vengono
  letti come UTC, quindi una pausa può essere alcune ore troppo lunga o troppo breve. Viene
  ricalcolata al prossimo tentativo reale.
- **La rilevazione è memorizzata nella cache per 10 minuti** affinché `status` e `init` rimangano
  istantanei. `next-harness doctor --refresh` ignora la cache; installare un harness ed eseguire
  `next-harness run` immediatamente potrebbe aver bisogno di quel refresh.
- **Il catalogo è uno snapshot.** Gli harness rinominano flag e binari; quando una
  sonda fallisce la voce mostra `non installato` invece di indovinare.
- **WSL è rilevato, mai guidato.**
- **Un harness senza output viene fermato dopo 120 s di silenzio** (più il `startup_grace_s` del
  catalogo) e dopo 30 minuti totali. Un'esecuzione fermata viene segnalata come *bloccata*,
  mai come *senza quota*.

### Cosa NextHarness non fa

- Non aggira quote, limiti di velocità, paywall o login.
- Non esegue un modello più economico alle tue spalle.
- Non legge, copia né sincronizza credenziali tra strumenti.
- Non guida un'applicazione grafica o una sessione di terminale umana.
- Non fa riuscire il tuo compito: si assicura solo che il prossimo harness gratuito
  sappia esattamente dove si è fermato il precedente.

### Contribuire

Segnalazioni di bug, una voce di catalogo o un `doctor --report` da una macchina non testata
sono i benvenuti. Vedi [CONTRIBUTING.md](CONTRIBUTING.md).

### Supporto

NextHarness è MIT e resta gratuito. Se ti ha fatto risparmiare un abbonamento:

- [GitHub Sponsors](https://github.com/sponsors/SammyYohan)
- [Ko-fi](https://ko-fi.com/SammyYohan)
- [Buy Me a Coffee](https://www.buymeacoffee.com/SammyYohan)

### Licenza

[MIT](LICENSE).

---

## Português

### Por que

Cada agente de programação te dá uma cota gratuita, e cada cota gratuita se esgota
no meio de uma tarefa. Trocar manualmente significa reler o diff, digitar a
solicitação novamente e esperar que a próxima ferramenta entenda onde a anterior parou.

NextHarness faz essa transferência. Inicia sua tarefa com o primeiro harness da sua
lista, percebe quando um harness está sem cota, anota o que foi feito e
inicia o próximo na mesma tarefa — com o mesmo arquivo de tarefa e a mesma
árvore de trabalho.

Não é um agente. Nunca conversa com um modelo, nunca lê um token, e nunca
finge ser uma das ferramentas que executa.

### Sumário

- [Como funciona](#como-funciona)
- [Instalação](#instalação)
- [Início rápido](#início-rápido)
- [Comandos](#comandos)
- [Por que um harness é pausado](#por-que-um-harness-é-pausado)
- [Regras de segurança](#regras-de-segurança)
- [Configuração](#configuração)
- [Idiomas](#idiomas)
- [Compatibilidade](#compatibilidade)
- [Limites honestos](#limites-honestos)
- [O que NextHarness não faz](#o-que-nextharness-não-faz)
- [Contribuir](#contribuir)
- [Suporte](#suporte)

### Como funciona

```text
  você ── next-harness run "adicionar modo escuro" ──▶ .next-harness/TASK.md
                                                       │
                    ordem do catálogo                   ▼
        ┌──────────────────▶ harness #1 (gratuito) ──▶ concluído ──▶ pronto
        │                       │
        │                       │ sem cota
        │                       ▼
        │            state.json: pausa #1 até o reinício (escrita atômica)
        │            .next-harness/HANDOFF.md: tarefa, git status, diff --stat,
        │                              fim da saída, "não refazer"
        │                       │
        └───────────────────────┘  o próximo harness recebe HANDOFF.md
                                                       │
                                              todos os candidatos esgotados
                                                       ▼
                              gatekeeper: pagar agora / esperar reinício / parar
```

A detecção, a decisão de cota e a transferência são todas locais. O único tráfego
de rede é aquele que seu harness já faz.

### Instalação

**Um comando** — baixa a versão mais recente, verifica o checksum SHA-256 e o
instala no seu `PATH`.

Windows (PowerShell):

```powershell
irm https://raw.githubusercontent.com/SammyYohan/NextHarness/main/install.ps1 | iex
```

macOS e Linux:

```sh
curl -fsSL https://raw.githubusercontent.com/SammyYohan/NextHarness/main/install.sh | sh
```

**Binário pré-compilado** — baixe o arquivo para sua plataforma em
[Releases](https://github.com/SammyYohan/NextHarness/releases), verifique o
`.sha256` ao lado e coloque `next-harness` no seu `PATH`.

**A partir do código-fonte** — você precisa do Rust 1.85 ou superior:

```sh
cargo install --git https://github.com/SammyYohan/NextHarness --bin next-harness
```

No Linux a compilação precisa dos cabeçalhos GTK e XDo, porque o aviso de pagamento é um
diálogo nativo:

```sh
sudo apt-get install libgtk-3-dev libxdo-dev
```

### Início rápido

```sh
cd ~/code/meu-projeto
next-harness init                          # detecta o que está instalado, escreve padrões
next-harness doctor                        # veja a ordem de transferência e o que falta
next-harness run "adicionar modo escuro e testes"
```

Esse é todo o ciclo. Se o primeiro harness acabar a cota, NextHarness o pausa,
escreve `.next-harness/HANDOFF.md` e inicia o próximo. Quando tudo está
esgotado pergunta: pagar, esperar o reinício ou parar.

Cada execução deixa seu rastro no repositório:

```text
.next-harness/
├── TASK.md      a tarefa, escrita uma vez
├── HANDOFF.md   reescrito antes de cada troca
└── runs/        transcrições por tentativa (ANSI removido, segredos mascarados)
```

### Comandos

| Comando | O que faz |
|---|---|
| `next-harness init` | Detecta harnesses instalados, imprime a ordem de transferência, escreve `config.json` |
| `next-harness run "<tarefa>"` | Executa uma tarefa, com transferência por cota |
| `next-harness resume` | Continua a tarefa já em `.next-harness/TASK.md` |
| `next-harness status` | Quais harnesses estão pausados e até quando |
| `next-harness doctor [--refresh] [--report]` | Inventário; `--report` imprime Markdown colável |
| `next-harness config` | Onde ficam a configuração e o estado |

Bandeiras úteis: `--lang pt|en|fr|es|de|it|zh|ja` em qualquer comando, `--max-attempts <n>` e `--manual` em `run` e `resume`.

### Por que um harness é pausado

Uma única linha contendo a palavra "cota" não prova nada — um agente pode estar
escrevendo código sobre cota. NextHarness só pausa quando **dois sinais
independentes** coincidem:

1. **Um padrão** nas últimas 40 linhas de saída, **e**
2. **Um segundo sinal**: o processo saiu com código diferente de zero, *ou* saiu com zero, imprimiu
   menos de 600 caracteres e não alterou nenhum arquivo segundo `git status`.

Harnesses cuja mensagem de limite de uso chega com código de saída 0 declaram
`quota_ignores_exit_code` no catálogo; para esses, "saída curta" é suficiente.

Qualquer coisa mais fraca abre uma pergunta em vez de pausar. Um tempo de reinício é tomado
da mensagem do próprio harness (`resets in 2 hours`, `try again in 30 seconds`,
`resets at 14:30`) e caso contrário, uma pausa de uma hora é aplicada.

### Regras de segurança

- **Um harness pago só inicia após um clique.** Qualquer nível diferente de `gratuito`
  — incluindo `desconhecido` — precisa de uma resposta explícita, em um terminal (pagar /
  esperar / cancelar, com um sinal sonoro) ou em um diálogo nativo.
- **Sem terminal e sem área de trabalho, o NextHarness espera ou para. Nunca paga.**
- **A interface de uma ferramenta interativa nunca é automatizada.** O NextHarness copia o
  prompt para a área de transferência, diz o que fazer e espera `next-harness resume`.
- **Sem telemetria.** Nada é enviado para lugar nenhum, exceto o que seu harness já envia.
- **Os segredos são mascarados** nos registros e no `HANDOFF.md`.
- **Uma conta por harness.** O NextHarness nunca lê, copia ou move credenciais.
- As pausas são persistidas de forma atômica: uma falha não pode gastar silenciosamente uma cota novamente.

### Configuração

- `profiles/builtin.toml` é o catálogo, compilado no binário.
- Substitua-o sem recompilar: coloque arquivos `*.toml` em
  `<config_dir>/profiles.d/`. São patches aplicados por `id` do harness.
- `<config_dir>/config.json` armazena `max_attempts` e `manual`, escritos por
  `next-harness init` e mostrados por `next-harness config`.
- `<config_dir>/state.json` armazena as pausas.

`<config_dir>` é o diretório de configuração da plataforma (`%APPDATA%` no Windows,
`~/.config` no Linux, `~/Library/Application Support` no macOS).

### Idiomas

O NextHarness está traduzido em:

| Código | Idioma |
|---|---|
| `en` | Inglês |
| `fr` | Francês |
| `es` | Espanhol |
| `de` | Alemão |
| `it` | Italiano |
| `pt` | Português |
| `zh` | Chinês simplificado |
| `ja` | Japonês |

Passe `--lang <código>` a qualquer comando, ou deixe o NextHarness escolher automaticamente
o idioma do seu sistema.

### Compatibilidade

`confidence` indica o quanto se pode confiar em uma entrada do catálogo: `official`
(documentação do fornecedor), `third party` (documentada por um wrapper ou a
comunidade) ou `unverified` (nunca testado em uma máquina real — por favor
[reporte o que você vê](.github/ISSUE_TEMPLATE/test-report.md)).

Uma entrada `gui` só é detectada pelo nome: o NextHarness não abre nenhum aplicativo de desktop
e não controla nenhuma janela.

### Limites honestos

- **Esta beta foi exercida no Windows.** Os caminhos de código do Linux e macOS
  (`PATH` do shell de login, varredura de `.desktop` e `/Applications`, fallback GTK)
  estão escritos e revisados, mas ainda não executados pelo autor; por isso várias
  entradas dizem `unverified`.
- **No Windows, a árvore de processos é morta com `taskkill /T /F`**, não com um job object: a
  lista de dependências desta versão não inclui um crate de API do Windows.
- **Os horários são mostrados em UTC.** Horários de reinício impressos por um harness em horário local são
  lidos como UTC, então uma pausa pode ser algumas horas muito longa ou curta. É
  recalculada na próxima tentativa real.
- **A detecção é armazenada em cache por 10 minutos** para que `status` e `init` permaneçam instantâneos.
  `next-harness doctor --refresh` ignora o cache; instalar um harness e executar
  `next-harness run` imediatamente pode precisar desse refresh.
- **O catálogo é um snapshot.** Os harnesses renomeiam flags e binários; quando uma
  sonda falha, a entrada mostra `não instalado` em vez de adivinhar.
- **WSL é detectado, nunca controlado.**
- **Um harness sem saída é parado após 120 s de silêncio** (mais o `startup_grace_s` do
  catálogo) e depois de 30 minutos no total. Uma execução parada é
  relatada como *bloqueada*, nunca como *sem cota*.

### O que NextHarness não faz

- Não ignora cotas, limites de taxa, paywalls ou logins.
- Não executa um modelo mais barato às suas costas.
- Não lê, copia nem sincroniza credenciais entre ferramentas.
- Não controla um aplicativo gráfico ou uma sessão de terminal conduzida por um humano.
- Não faz sua tarefa ter sucesso: apenas garante que o próximo harness gratuito
  saiba exatamente onde o anterior parou.

### Contribuir

Relatórios de bugs, uma entrada de catálogo ou um `doctor --report` de uma máquina não testada
são bem-vindos. Veja [CONTRIBUTING.md](CONTRIBUTING.md).

### Suporte

O NextHarness é MIT e permanece gratuito. Se ele salvou uma assinatura para você:

- [GitHub Sponsors](https://github.com/sponsors/SammyYohan)
- [Ko-fi](https://ko-fi.com/SammyYohan)
- [Buy Me a Coffee](https://www.buymeacoffee.com/SammyYohan)

### Licença

[MIT](LICENSE).

---

## 中文（简体）

### 为什么

每个代理 AI 都会给你一个免费额度，而每个免费额度都会在任务中途耗尽。手动切换意味着重新阅读 diff，重新输入请求，并希望下一个工具能理解上一个停止的地方。

NextHarness 完成了这一交接。它以列表中的第一个 harness 开始你的任务，注意到某个 harness 额度已用尽，记录已完成的工作，并在下一个 harness 上继续同一任务——使用相同的任务文件和相同的工作树。

它不是代理。它从不与模型对话，从不读取任何 token，也不假装是它所驱动的任何工具。

### 目录

- [工作原理](#工作原理)
- [安装](#安装)
- [快速入门](#快速入门)
- [命令](#命令)
- [为什么 harness 会暂停](#为什么-harness-会暂停)
- [安全规则](#安全规则)
- [配置](#配置)
- [语言](#语言)
- [兼容性](#兼容性)
- [诚实的限制](#诚实的限制)
- [NextHarness 不做什么](#nextharness-不做什么)
- [贡献](#贡献)
- [支持](#支持)

### 工作原理

```text
  你 ── next-harness run "添加深色模式" ──▶ .next-harness/TASK.md
                                                │
                      目录顺序                    ▼
        ┌──────────────────▶ harness #1 (免费) ──▶ 完成 ──▶ 完成
        │                       │
        │                       │ 额度耗尽
        │                       ▼
        │            state.json: 暂停 #1 直到重置 (原子写入)
        │            .next-harness/HANDOFF.md: 任务, git status, diff --stat,
        │                              输出末尾, "不要重做"
        │                       │
        └───────────────────────┘  下一个 harness 获取 HANDOFF.md
                                                │
                                      所有候选者耗尽
                                                ▼
                              gatekeeper: 立即付费 / 等待重置 / 停止
```

检测、配额决策和交接均在本地完成。唯一的网络流量是你的 harness 已经产生的流量。

### 安装

**一条命令** — 下载最新版本，校验 SHA-256 校验和并安装到你的 `PATH`。Windows（PowerShell）：

```powershell
irm https://raw.githubusercontent.com/SammyYohan/NextHarness/main/install.ps1 | iex
```

macOS 和 Linux：

```sh
curl -fsSL https://raw.githubusercontent.com/SammyYohan/NextHarness/main/install.sh | sh
```

**预编译二进制文件** — 从 [Releases](https://github.com/SammyYohan/NextHarness/releases) 下载适用于你平台的存档，验证旁边的 `.sha256`，并将 `next-harness` 放入你的 `PATH`。

**从源代码编译** — 你需要 Rust 1.85 或更高版本：

```sh
cargo install --git https://github.com/SammyYohan/NextHarness --bin next-harness
```

在 Linux 上，编译需要 GTK 和 XDo 头文件，因为付费提示是原生对话框：

```sh
sudo apt-get install libgtk-3-dev libxdo-dev
```

### 快速入门

```sh
cd ~/code/my-project
next-harness init                          # 检测已安装的 harness，写入默认值
next-harness doctor                        # 查看交接顺序和缺失内容
next-harness run "添加深色模式和测试"
```

这就是全部循环。如果第一个 harness 额度耗尽，NextHarness 会暂停它，
写入 `.next-harness/HANDOFF.md`，并启动下一个。当所有都耗尽时，它会问：付费，等待重置，或停止。

每次运行都会在仓库中留下痕迹：

```text
.next-harness/
├── TASK.md      任务，写入一次
├── HANDOFF.md   每次切换前重写
└── runs/        每次尝试的转录（去除 ANSI，隐藏密钥）
```

### 命令

| 命令 | 作用 |
|---|---|
| `next-harness init` | 检测已安装的 harness，打印交接顺序，写入 `config.json` |
| `next-harness run "<任务>"` | 运行任务，基于配额交接 |
| `next-harness resume` | 继续 `.next-harness/TASK.md` 中已存在的任务 |
| `next-harness status` | 哪些 harness 暂停，以及直到何时 |
| `next-harness doctor [--refresh] [--report]` | 清单；`--report` 打印可粘贴的 Markdown |
| `next-harness config` | 配置和状态所在位置 |

有用标志：对任何命令使用 `--lang zh|en|fr|es|de|it|pt|ja`，以及 `run` 和 `resume` 上的 `--max-attempts <n>` 和 `--manual`。

### 为什么 harness 会暂停

一条包含 "quota" 这个词的单行并不能证明什么 — 代理可能正在编写关于配额的代码。NextHarness 仅在 **两个独立信号** 一致时才暂停：

1. **最近 40 行输出中的一个模式**，**和**
2. **第二个信号**：进程退出码非零，*或* 退出码为零、输出少于 600 个字符且根据 `git status` 没有修改文件。

那些用限制消息带退出码 0 到达的 harness 在目录中声明 `quota_ignores_exit_code`；对于这些，"短输出" 就足够了。

任何更弱的情况都会打开提示而不是暂停。重置时间来自 harness 的自身消息（`resets in 2 hours`、`try again in 30 seconds`、`resets at 14:30`），否则应用一小时暂停。

### 安全规则

- **付费 harness 只有在点击后才会启动。** 任何非 `free` 的级别——包括 `unknown`——都需要明确的答案，在终端中（付费 / 等待 / 停止，伴随铃声）或在原生对话框中。
- **没有终端和桌面时，NextHarness 会等待或停止。它从不付费。**
- **交互式工具的界面永不自动化。** NextHarness 将提示复制到剪贴板，告诉你该做什么，并等待 `next-harness resume`。
- **无遥测。** 不会发送任何内容，除了你的 harness 已经发送的内容。
- **密钥在日志和 HANDOFF.md 中被掩码。**
- **每个 harness 一个账户。** NextHarness 从不读取、复制或移动凭据。
- 暂停被原子化持久化：崩溃不会悄悄重复花费配额。

### 配置

- `profiles/builtin.toml` 是目录，编译进二进制文件。
- 无需重新编译即可替代它：将 `*.toml` 文件放入 `<config_dir>/profiles.d/`。它们是按 harness `id` 应用的补丁。
- `<config_dir>/config.json` 保存 `max_attempts` 和 `manual`，由 `next-harness init` 写入，由 `next-harness config` 显示。
- `<config_dir>/state.json` 保存暂停。

`<config_dir>` 是平台配置目录（Windows 上的 `%APPDATA%`、Linux 上的 `~/.config`、macOS 上的 `~/Library/Application Support`）。

### 语言

NextHarness 已翻译成：

| 代码 | 语言 |
|---|---|
| `en` | 英语 |
| `fr` | 法语 |
| `es` | 西班牙语 |
| `de` | 德语 |
| `it` | 意大利语 |
| `pt` | 葡萄牙语 |
| `zh` | 中文（简体） |
| `ja` | 日语 |

向任何命令传递 `--lang <代码>`，或让 NextHarness 自动选择你系统的语言。

### 兼容性

`confidence` 表示目录条目可信度：`official`（供应商文档）、`third party`（由包装器或社区文档化）或 `unverified`（从未在真实机器上测试——请 [报告你看到的内容](.github/ISSUE_TEMPLATE/test-report.md)）。

`gui` 条目仅按名称检测：NextHarness 不会打开任何桌面应用程序，也不会驱动任何窗口。

### 诚实的限制

- **此版本已在 Windows 上验证。** Linux 和 macOS 代码路径（登录 shell `PATH`、`.desktop` 和 `/Applications` 扫描、GTK 回退）已编写和评审，但尚未由作者运行；因此多个条目标记为 `unverified`。
- **Windows 上进程树终止使用 `taskkill /T /F`**，而非作业对象：此版本的依赖列表不包括 Windows API crate。
- **时间显示为 UTC。** harness 在本地时间中打印的重置时间被读作 UTC，因此暂停可能比实际长或短几小时。它会在下一次真实尝试时重新计算。
- **检测缓存 10 分钟**，以便 `status` 和 `init` 保持即时。`next-harness doctor --refresh` 忽略缓存；安装一个 harness 后立即运行 `next-harness run` 可能需要该刷新。
- **目录是快照。** harness 会重命名标志和二进制文件；当探测失败时，条目显示 `未安装` 而非猜测。
- **WSL 被检测，从不驱动。**
- **没有输出的代理在 120 s 沉默后停止**（加上目录中的 `startup_grace_s`），以及总共 30 分钟后。停止的运行报告为 *阻塞*，从不报告为 *额度耗尽*。

### NextHarness 不做什么

- 它不绕过配额、速率限制、付费墙或登录。
- 它不会背后运行更便宜的模型。
- 它不读取、复制或同步工具之间的凭据。
- 它不驱动图形应用程序或人工驱动的终端会话。
- 它不会让你的任务成功：它只确保下一个免费 harness 知道上一个在哪里停止。

### 贡献

错误报告、一个目录条目，或来自未测试机器的 `doctor --report` 都受欢迎。参见 [CONTRIBUTING.md](CONTRIBUTING.md)。

### 支持

NextHarness 是 MIT 协议且永久免费。如果它为你节省了订阅费：

- [GitHub Sponsors](https://github.com/sponsors/SammyYohan)
- [Ko-fi](https://ko-fi.com/SammyYohan)
- [Buy Me a Coffee](https://www.buymeacoffee.com/SammyYohan)

### 许可证

[MIT](LICENSE).

---

## 日本語

### なぜ

すべてのコーディングエージェントは無料枠を提供していますが、その無料枠はタスクの途中で使い果たされます。手動で切り替えるには、diff を再度読み直し、リクエストを再入力し、次のツールが前のツールの続きを理解することを願う必要があります。

NextHarness がその引き継ぎを行います。リストの最初のハーネスでタスクを開始し、ハーネスが無料枠を使い果たしたときに検出し、何が行われたかを記録し、同じタスクファイルと同じワークツリーで次のハーネスを開始します。

これはエージェントではありません。モデルと対話せず、トークンを読まず、駆動するツールのいずれかになりすましません。

### 目次

- [仕組み](#仕組み)
- [インストール](#インストール)
- [クイックスタート](#クイックスタート)
- [コマンド](#コマンド)
- [ハーネスが一時停止される理由](#ハーネスが一時停止される理由)
- [安全ルール](#安全ルール)
- [設定](#設定)
- [言語](#言語)
- [互換性](#互換性)
- [正直な制限](#正直な制限)
- [NextHarness がしないこと](#nextharness-がしないこと)
- [貢献](#貢献)
- [サポート](#サポート)

### 仕組み

```text
  あなた ── next-harness run "ダークモードを追加" ──▶ .next-harness/TASK.md
                                                      │
                      カタログ順序                      ▼
        ┌──────────────────▶ ハーネス #1 (無料) ──▶ 完了 ──▶ 完了
        │                       │
        │                       │ 無料枠が尽きる
        │                       ▼
        │            state.json: #1 をリセットまで一時停止 (原子書き込み)
        │            .next-harness/HANDOFF.md: タスク, git status, diff --stat,
        │                              出力の末尾, "やり直し禁止"
        │                       │
        └───────────────────────┘  次のハーネスが HANDOFF.md を受け取る
                                                      │
                                              すべての候補が尽きる
                                                      ▼
                              gatekeeper: 今すぐ支払う / リセットを待つ / 停止する
```

検出、無料枠の判断、引き継ぎはすべてローカルで行われます。ネットワークトラフィックは、お使いのハーネスが既に発生させているもののみです。

### インストール

**ワンコマンド** — 最新リリースをダウンロードし、SHA-256 を検証して、`PATH` にインストールします。Windows（PowerShell）：

```powershell
irm https://raw.githubusercontent.com/SammyYohan/NextHarness/main/install.ps1 | iex
```

macOS と Linux：

```sh
curl -fsSL https://raw.githubusercontent.com/SammyYohan/NextHarness/main/install.sh | sh
```

**事前ビルド済みバイナリ** — [Releases](https://github.com/SammyYohan/NextHarness/releases) からプラットフォーム用のアーカイブをダウンロードし、隣の `.sha256` を確認し、`next-harness` を `PATH` に配置してください。

**ソースからビルド** — Rust 1.85 以降が必要です：

```sh
cargo install --git https://github.com/SammyYohan/NextHarness --bin next-harness
```

Linux では、ビルドに GTK と XDo のヘッダーが必要です。これは支払いプロンプトがネイティブダイアログであるためです：

```sh
sudo apt-get install libgtk-3-dev libxdo-dev
```

### クイックスタート

```sh
cd ~/code/my-project
next-harness init                          # インストール済みハーネスを検出し、デフォルトを書き込む
next-harness doctor                        # 引き継ぎ順序と不足しているものを確認する
next-harness run "ダークモードとテストを追加"
```

これが全工程です。最初のハーネスが無料枠を使い果たすと、NextHarness はそれを一時停止し、`.next-harness/HANDOFF.md` を書き込み、次のハーネスを開始します。すべて尽きると、支払い、リセット待ち、または停止を尋ねます。

すべての実行はリポジトリに痕跡を残します：

```text
.next-harness/
├── TASK.md      タスク (一度だけ書き込む)
├── HANDOFF.md   切り替えのたびに書き直す
└── runs/        1 試行ごとのトランスクリプト (ANSI を除去、シークレットをマスク)
```

### コマンド

| コマンド | 説明 |
|---|---|
| `next-harness init` | インストール済みハーネスを検出し、引き継ぎ順序を表示し、`config.json` を書き込む |
| `next-harness run "<タスク>"` | タスクを実行し、無料枠に応じて引き継ぐ |
| `next-harness resume` | `.next-harness/TASK.md` に保存されているタスクを続行する |
| `next-harness status` | 一時停止中のハーネスと再開時刻を表示する |
| `next-harness doctor [--refresh] [--report]` | インベントリ。`--report` は貼り付け可能な Markdown を出力する |
| `next-harness config` | 設定と状態の保存場所を表示する |

便利なフラグ: すべてのコマンドで `--lang ja|en|fr|es|de|it|pt|zh`、`run` と `resume` で `--max-attempts <n>` と `--manual`。

### ハーネスが一時停止される理由

"quota" という単語が含まれる 1 行だけでは何も証明されません。エージェントがクォータについてコードを書いている可能性があります。NextHarness は **2 つの独立したシグナル** が一致した場合にのみ一時停止します:

1. **出力の最後 40 行にパターンがあること**、**と**
2. **2 つ目のシグナル**: プロセスが 0 以外の終了コードで終了した、*または* 終了コード 0、600 文字未満の出力、そして `git status` でファイルが変更されていないこと。

終了コード 0 で使用制限メッセージが到着するハーネスはカタログに `quota_ignores_exit_code` を宣言しています。これらについては、「短い出力」で十分です。

これより弱い場合は、一時停止せずにユーザーに確認を求めます。リセット時刻はハーネス自身のメッセージ（`resets in 2 hours`、`try again in 30 seconds`、`resets at 14:30`）から取得され、それ以外の場合は 1 時間の一時停止が適用されます。

### 安全ルール

- **有料ハーネスは、ユーザーのクリック後のみ開始されます。** `free` 以外のすべての階層（`unknown` を含む）は、明確な回答（支払い / 待機 / 中止、音付きターミナルまたはネイティブダイアログ）が必要です。
- **ターミナルもデスクトップがない場合、NextHarness は待機するか停止します。決して支払いません。**
- **インタラクティブなツールのインターフェイスは決して自動化されません。** NextHarness はプロンプトをクリップボードにコピーし、何をすべきかを伝え、`next-harness resume` を待ちます。
- **テレメトリはありません。** ハーネスが既に送信しているもの以外、何も送信されません。
- **シークレットはログと `HANDOFF.md` にマスクされます。**
- **ハーネスごとに 1 つのアカウント。** NextHarness は認証情報を読み取ったり、コピーしたり、移動したりしません。
- 一時停止は原子的に永続化されるため、クラッシュしても静かにクォータを再消費することはありません。

### 設定

- `profiles/builtin.toml` はバイナリにコンパイルされたカタログです。
- 再コンパイルせずに上書きするには、`<config_dir>/profiles.d/` に `*.toml` ファイルを置いてください。これらはハーネス ID ごとに適用されるパッチです。
- `<config_dir>/config.json` には `max_attempts` と `manual` が保存され、`next-harness init` で書き込まれ、`next-harness config` で表示されます。
- `<config_dir>/state.json` には一時停止が保存されます。

`<config_dir>` はプラットフォームの構成ディレクトリ（Windows の `%APPDATA%`、Linux の `~/.config`、macOS の `~/Library/Application Support`）です。

### 言語

NextHarness は以下の言語に翻訳されています:

| コード | 言語 |
|---|---|
| `en` | 英語 |
| `fr` | フランス語 |
| `es` | スペイン語 |
| `de` | ドイツ語 |
| `it` | イタリア語 |
| `pt` | ポルトガル語 |
| `zh` | 中国語（簡体） |
| `ja` | 日本語 |

任意のコマンドに `--lang <コード>` を渡すか、システムの言語を NextHarness に自動選択させることができます。

### 互換性

`confidence` はカタログエントリの信頼度を示します: `official`（ベンダーの公式ドキュメント）、`third party`（ラッパーまたはコミュニティによる文書）、`unverified`（実機で未テスト—[実機での結果を報告](.github/ISSUE_TEMPLATE/test-report.md) してください）。

`gui` エントリは名前によってのみ検出されます。NextHarness はデスクトップアプリケーションを開かず、ウィンドウを操作しません。

### 正直な制限

- **このベータ版は Windows で検証済みです。** Linux と macOS のコードパス（ログインシェルの `PATH`、`.desktop` と `/Applications` のスキャン、GTK フォールバック）は作成・レビュー済みですが、作者によってまだ実行されていません。そのためいくつかのエントリは `unverified` と記載されています。
- **Windows ではプロセスツリーの終了に `taskkill /T /F` を使用します。** ジョブオブジェクトではありません。このバージョンの依存関係リストには Windows API クレートは含まれていません。
- **時刻は UTC で表示されます。** ハーネスがローカルタイムで出力するリセット時刻は UTC として読み取られるため、一時停止時間が数時間長すぎたり短すぎたりする可能性があります。次の実際の試行時に再計算されます。
- **検出は 10 分間キャッシュされます。** `status` と `init` を即時に保つためです。`next-harness doctor --refresh` はキャッシュを無視します。ハーネスをインストールしてすぐに `next-harness run` を実行する場合、このリフレッシュが必要になることがあります。
- **カタログはスナップショットです。** ハーネスはフラグやバイナリ名を変更することがあります。プローブが失敗した場合、エントリは `インストールされていません` と表示され、推測は行われません。
- **WSL は検出されますが、決して操作されません。**
- **出力のないハーネスは 120 秒の沈黙後（カタログの `startup_grace_s` を加算）に停止され、合計 30 分後に停止されます。** 停止された実行は *ブロック* として報告され、決して *クォータ切れ* として報告されません。

### NextHarness がしないこと

- クォータ、レート制限、ペイウォール、ログインを回避しません。
- あなたの知らない間に安価なモデルを実行しません。
- ツール間で認証情報を読み取り、コピー、同期しません。
- グラフィカルアプリケーションや人間が操作するターミナルセッションを駆動しません。
- あなたのタスクを成功させるわけではありません。次の無料ハーネスが前のハーネスが停止した場所を正確に知ることを保証するだけです。

### 貢献

バグレポート、カタログエントリ、未テストマシンからの `doctor --report` は歓迎します。[CONTRIBUTING.md](CONTRIBUTING.md) を参照してください。

### サポート

NextHarness は MIT ライセンスで、無料のままです。サブスクリプションを節約できた場合は:

- [GitHub Sponsors](https://github.com/sponsors/SammyYohan)
- [Ko-fi](https://ko-fi.com/SammyYohan)
- [Buy Me a Coffee](https://www.buymeacoffee.com/SammyYohan)

### ライセンス

[MIT](LICENSE).
