Voici un blueprint complet pour agentd, séparé en 2 grands blocs :
	•	CLI
	•	Runtime

L’idée est de te donner quelque chose de structuré comme une vraie spec produit, pas juste une liste d’idées.

⸻

Vision globale

agentd peut devenir :

une plateforme CLI d’orchestration d’agents autonomes, avec runtime sécurisé intégré, mémoire, RAG, Git-aware execution, scheduling, audit et plugins

En plus simple :
	•	le CLI = control plane
	•	le runtime = execution plane
	•	SQLite = état local / audit / scheduling
	•	providers = accès LLM / HTTP / outils
	•	plans = workflows agentiques
	•	runtime = shell intelligent contrôlé

⸻

1. Blueprint global du produit

1.1 Piliers du système

A. Orchestration

Le CLI crée, planifie, exécute, supervise des agents.

B. Exécution

Le runtime exécute les actions de l’agent dans un workspace contrôlé.

C. Connaissance

Le système peut chercher de l’info locale/externe via RAG, mémoire, search repo.

D. Sécurité

Permissions, sandbox, policies, audit trail.

E. Observabilité

Logs, traces, replay, scoring, explication.

F. Autonomie

Plans adaptatifs, scheduler, triggers, self-reflection.

⸻

2. Blueprint CLI

⸻

2.1 Objectif du CLI

Le CLI doit servir à 6 usages :
	•	lancer un agent ponctuel
	•	exécuter un plan
	•	gérer des sessions runtime
	•	interroger les runs/logs/diffs
	•	planifier des agents
	•	administrer la plateforme locale

⸻

2.2 Architecture logique du CLI

Modules CLI
	•	agent
	•	plan
	•	run
	•	runtime
	•	schedule
	•	memory
	•	knowledge
	•	logs
	•	git
	•	policy
	•	plugin
	•	config
	•	eval
	•	debug
	•	doctor

⸻

2.3 Command groups recommandés

⸻

A. agent

Gestion des agents.

Exemples :

agentd agent create
agentd agent list
agentd agent show <id>
agentd agent run <id>
agentd agent stop <id>
agentd agent resume <id>
agentd agent clone <id>
agentd agent delete <id>

Fonctions :
	•	créer un agent
	•	définir son provider
	•	définir son profil runtime
	•	attacher mémoire / knowledge sources
	•	définir permissions et objectifs

⸻

B. plan

Plans YAML/JSON et workflows.

agentd plan generate
agentd plan validate ./plans/fix-tests.yaml
agentd plan run ./plans/fix-tests.yaml
agentd plan explain ./plans/fix-tests.yaml
agentd plan list
agentd plan show <id>

Fonctions :
	•	valider un plan
	•	simuler le flow
	•	exécuter
	•	inspecter les steps
	•	générer un plan depuis un objectif

⸻

C. run

Exécution et supervision des runs.

agentd run start --agent coder --goal "fix failing tests"
agentd run list
agentd run show <run_id>
agentd run cancel <run_id>
agentd run retry <run_id>
agentd run resume <run_id>
agentd run explain <run_id>
agentd run export <run_id>

Fonctions :
	•	démarrer un run
	•	inspecter son état
	•	relancer
	•	exporter résultats/logs/diffs
	•	rejouer

⸻

D. runtime

Gestion du runtime intégré.

agentd runtime start
agentd runtime list
agentd runtime show <session_id>
agentd runtime exec <session_id> -- git status
agentd runtime attach <session_id>
agentd runtime stop <session_id>
agentd runtime snapshot <session_id>
agentd runtime rollback <session_id>

Fonctions :
	•	démarrer une session isolée
	•	exécuter des commandes manuellement
	•	voir état du workspace
	•	rollback
	•	snapshot

⸻

E. schedule

Planification et autonomie.

agentd schedule at "2026-03-25T09:00:00Z" --agent daily-review
agentd schedule cron "0 * * * *" --plan ./plans/reindex.yaml
agentd schedule list
agentd schedule show <job_id>
agentd schedule cancel <job_id>
agentd schedule dispatch-due

Fonctions :
	•	cron
	•	one-shot
	•	recurring
	•	dispatch manuel
	•	jobs autonomes

⸻

F. memory

Mémoire agentique.

agentd memory add
agentd memory list
agentd memory search "previous auth bug"
agentd memory show <memory_id>
agentd memory prune
agentd memory export

Types de mémoire :
	•	short-term
	•	episodic
	•	semantic
	•	tool memory
	•	decisions
	•	facts

⸻

G. knowledge

RAG / search / indexation.

agentd knowledge ingest ./docs
agentd knowledge sources list
agentd knowledge search "how auth refresh works"
agentd knowledge reindex
agentd knowledge stats
agentd knowledge verify

Fonctions :
	•	ingestion
	•	recherche hybride
	•	indexation
	•	freshness check
	•	source verification

⸻

H. logs

Logs et audit.

agentd logs tail
agentd logs run <run_id>
agentd logs runtime <session_id>
agentd logs search "permission denied"
agentd logs export <run_id>

Fonctions :
	•	recherche dans les logs
	•	export JSONL
	•	filtrage par run/session/agent

⸻

I. git

Couche Git de haut niveau.

agentd git status <session_id>
agentd git diff <session_id>
agentd git diff --against-base <session_id>
agentd git commit <session_id> -m "fix auth tests"
agentd git branch <session_id>
agentd git merge-preview <session_id>
agentd git merge <session_id>
agentd git rollback <session_id>


⸻

J. policy

Permissions et sécurité.

agentd policy list
agentd policy show dev-safe
agentd policy validate ./agentd.toml
agentd policy test --agent coder --command "git push"
agentd policy dry-run


⸻

K. plugin

Extensibilité.

agentd plugin list
agentd plugin install rag
agentd plugin remove rag
agentd plugin enable rag
agentd plugin disable rag
agentd plugin info rag


⸻

L. eval

Évaluation.

agentd eval run benchmark.yaml
agentd eval rag
agentd eval runtime
agentd eval compare run1 run2

Métriques :
	•	réussite
	•	groundedness
	•	latence
	•	coût
	•	sécurité
	•	qualité retrieval
	•	diff quality

⸻

M. debug

Débug profond.

agentd debug run <id>
agentd debug replay <id>
agentd debug trace <id>
agentd debug prompt <id>
agentd debug state <id>


⸻

N. doctor

Santé système.

agentd doctor
agentd doctor runtime
agentd doctor db
agentd doctor config
agentd doctor permissions


⸻

2.4 Features CLI à ajouter

⸻

A. UX / ergonomie
	•	sous-commandes cohérentes
	•	mode interactif TUI
	•	autocompletion shell
	•	help contextuel
	•	profils par défaut
	•	presets de plans
	•	commandes alias
	•	config locale + globale
	•	output human / json / yaml
	•	support --watch
	•	--dry-run partout où possible

⸻

B. Gestion d’agents
	•	templates d’agents
	•	clonage d’agent
	•	versioning d’agent
	•	profiles provider/runtime
	•	capabilities attachées à l’agent
	•	objectifs persistants
	•	historique des runs
	•	score de performance par agent

⸻

C. Plans et workflows
	•	conditionals
	•	loops
	•	retries
	•	fan-out / fan-in
	•	branching
	•	subplans
	•	includes
	•	variables et interpolation
	•	policy checks par step
	•	timeout par step
	•	output contracts

⸻

D. Scheduling & autonomie
	•	cron
	•	event triggers
	•	file triggers
	•	webhooks
	•	polling jobs
	•	recurring maintenance tasks
	•	auto resume après reboot
	•	dead letter queue
	•	priority queue

⸻

E. Knowledge / mémoire
	•	mémoire épisodique
	•	mémoire sémantique
	•	index repo local
	•	search docs internes
	•	RAG hybride
	•	citations
	•	freshness score
	•	knowledge source trust levels
	•	dedupe de chunks
	•	memory pruning policies

⸻

F. Observabilité
	•	timeline run
	•	explain mode
	•	structured traces
	•	run comparison
	•	token usage
	•	command latency
	•	permission denial history
	•	failure classification
	•	quality scoring
	•	artifact browser

⸻

G. Sécurité
	•	policy simulation
	•	mode approval required
	•	audit export
	•	secret masking
	•	command blocklists
	•	path protections
	•	env whitelisting
	•	network policy
	•	profile isolation
	•	signed action logs

⸻

H. DevEx
	•	replay
	•	deterministic test mode
	•	mock providers
	•	fake runtime
	•	fixtures de sessions
	•	run snapshots
	•	export/import de state
	•	benchmark CLI
	•	prompt inspection
	•	structured errors

⸻

2.5 Data model CLI / control plane

SQLite local.

Tables proposées
	•	agents
	•	agent_versions
	•	runs
	•	run_steps
	•	plans
	•	plan_versions
	•	schedules
	•	schedule_runs
	•	memories
	•	facts
	•	knowledge_sources
	•	knowledge_documents
	•	knowledge_chunks
	•	retrieval_runs
	•	runtime_sessions
	•	runtime_events
	•	artifacts
	•	git_snapshots
	•	policy_profiles
	•	permission_decisions
	•	plugins
	•	evaluations

⸻

2.6 Output modes du CLI

Chaque commande peut rendre :
	•	human
	•	json
	•	jsonl
	•	yaml
	•	compact
	•	debug

Exemple :

agentd run show abc --output json
agentd logs tail --output compact


⸻

2.7 Grands modes produit du CLI

Mode 1 — local solo

Tout local, SQLite, runtime intégré.

Mode 2 — team

Config partagée, plans partagés, policies partagées.

Mode 3 — CI / automation

Commandes non interactives, output structuré, deterministic mode.

⸻

3. Blueprint Runtime

⸻

3.1 Objectif du runtime

Le runtime est un environnement d’exécution agentique contrôlé.

Il doit permettre :
	•	spawn d’une session
	•	exécution de commandes
	•	lecture/écriture de fichiers
	•	opérations Git
	•	logs complets
	•	permissions fines
	•	isolation workspace
	•	proxy de sortie intelligent type RTK
	•	replay / audit / rollback

⸻

3.2 Composants du runtime

A. RuntimeManager

Crée et gère les sessions.

B. RuntimeSession

Contexte vivant :
	•	session id
	•	workspace
	•	branch
	•	policy
	•	env
	•	logs
	•	state machine

C. CommandExecutor

Exécution réelle des commandes.

D. PermissionEvaluator

Décision allow / deny / ask.

E. WorkspaceGuard

Contrôle accès fichiers et chemins.

F. GitController

Abstraction Git.

G. OutputAdapter

Compression/summarization des sorties.

H. ArtifactStore

Stockage stdout/stderr, diffs, patches, snapshots.

I. SessionMemory

Historique local de la session.

J. RuntimeObserver

Collecte traces, metrics, anomalies.

⸻

3.3 Modes de runtime

1. local

Travaille dans le repo réel.

2. worktree

Branche + worktree dédié. Mode par défaut recommandé.

3. sandbox

Copie isolée locale.

4. container

Plus tard : Docker/OCI.

5. ephemeral

Session jetable.

⸻

3.4 Cycle de vie d’une session runtime
	1.	init
	2.	resolve repo/workspace
	3.	apply policy profile
	4.	snapshot base state
	5.	create branch/worktree if needed
	6.	session ready
	7.	exec commands
	8.	collect logs/artifacts
	9.	compute diff
	10.	optionally commit / merge proposal
	11.	close / persist / cleanup

⸻

3.5 State machine runtime

États recommandés :
	•	INIT
	•	READY
	•	RUNNING
	•	DIRTY
	•	TEST_FAILING
	•	BLOCKED
	•	AWAITING_APPROVAL
	•	MERGE_READY
	•	COMPLETED
	•	ROLLED_BACK
	•	FAILED

⸻

3.6 Features runtime à ajouter

⸻

A. Exécution de commandes
	•	shell exec contrôlé
	•	exec non-shell
	•	timeout
	•	memory limit
	•	output limit
	•	concurrency control
	•	retries
	•	cancellation
	•	signal handling
	•	command caching
	•	deterministic mock mode

⸻

B. File system intelligent
	•	read_file contrôlé
	•	write_file contrôlé
	•	append
	•	patch apply
	•	smart read
	•	chunked read
	•	path canonicalization
	•	symlink resolution
	•	file snapshots
	•	rollback file-level
	•	sensitive file protection
	•	change previews

⸻

C. Permissions / security
	•	allow/deny/ask
	•	capabilities-based model
	•	command profiles
	•	path-based permissions
	•	env whitelist
	•	network permissions
	•	git permissions
	•	merge permissions
	•	deletion permissions
	•	secret masking
	•	anomaly detection
	•	risk scoring
	•	command reputation classes

⸻

D. Output proxy type RTK++

Le runtime doit compresser les sorties avant de les donner au LLM.

Stratégies
	•	failures only
	•	stats only
	•	summary + snippets
	•	dedupe repeated lines
	•	group by file
	•	group by rule
	•	compact json
	•	structured errors
	•	head/tail windows
	•	progressive reveal
	•	tee raw logs to disk

Adapters spécifiques
	•	git status
	•	git diff
	•	git log
	•	pytest
	•	cargo test
	•	npm test
	•	eslint
	•	tsc
	•	docker logs
	•	kubectl logs
	•	rg
	•	cat
	•	ls
	•	find

⸻

E. Git-native runtime
	•	detect repo root
	•	capture base commit
	•	create session branch
	•	worktree support
	•	status
	•	diff working
	•	diff against base
	•	stage selected files
	•	commit with generated message
	•	commit grouping
	•	squash support
	•	merge preview
	•	assisted merge
	•	conflict summarization
	•	rollback to base
	•	revert patch
	•	snapshot diff history

⸻

F. Runtime memory

Le runtime se souvient de :
	•	commandes exécutées
	•	outputs importants
	•	fichiers touchés
	•	erreurs récurrentes
	•	tests déjà lancés
	•	diffs précédents
	•	décisions de permission
	•	stratégies gagnantes

⸻

G. Cognitive runtime

Le runtime devient context-aware.

Exemples :
	•	si pytest échoue → proposer les fichiers touchés et tests concernés
	•	si git diff énorme → résumer par module
	•	si npm install échoue → isoler réseau vs lockfile vs peer dependency
	•	si conflit merge → extraire hunks critiques

⸻

H. Session artifacts

Pour chaque session :
	•	stdout brut
	•	stderr brut
	•	sortie compacte
	•	diff snapshots
	•	patches
	•	fichiers modifiés
	•	tests exécutés
	•	policy decisions
	•	env used
	•	command traces

⸻

I. Replay / audit
	•	replay complet
	•	replay partiel
	•	compare runs
	•	diff between sessions
	•	explain last action
	•	provenance tracking
	•	signed logs optionnels

⸻

J. Approvals / human-in-the-loop
	•	ask before risky action
	•	ask before network
	•	ask before delete
	•	ask before merge
	•	ask before touching protected files
	•	show preview before approval

⸻

3.7 Modèle de permissions recommandé

Par capacités.

Capacités
	•	ReadFile
	•	WriteFile
	•	PatchFile
	•	DeleteFile
	•	ExecShell
	•	ExecBinary
	•	ExecGitRead
	•	ExecGitWrite
	•	ExecTests
	•	ExecNetwork
	•	ManageBranch
	•	MergeBranch
	•	ReadSecrets
	•	ModifyConfig
	•	CreateProcess
	•	KillProcess

Décision
	•	Allow
	•	Deny
	•	Ask

Scope
	•	agent
	•	runtime profile
	•	command
	•	path
	•	repo
	•	environment

⸻

3.8 Policy profiles runtime

read-only
	•	lecture repo
	•	git read only
	•	pas d’écriture
	•	pas de réseau

dev-safe
	•	lecture/écriture repo
	•	tests autorisés
	•	git commit autorisé
	•	merge interdit
	•	réseau limité

repo-maintainer
	•	git branch
	•	commit
	•	merge assisté
	•	accès configs limité

full-trusted
	•	quasi complet, pour usage local assumé

⸻

3.9 Workspace management

Workspace modes
	•	direct repo
	•	worktree
	•	temp copy
	•	container mount

Features
	•	auto cleanup
	•	cache dependencies
	•	snapshot on start
	•	snapshot on close
	•	quota size
	•	temp files garbage collection

⸻

3.10 Runtime commands virtuelles de haut niveau

Très utile pour éviter que l’agent improvise trop avec le shell.

Familles de commandes

Repo
	•	repo.status
	•	repo.diff
	•	repo.search
	•	repo.read_file
	•	repo.write_file
	•	repo.apply_patch
	•	repo.test
	•	repo.lint

Git
	•	git.status
	•	git.diff_base
	•	git.commit
	•	git.create_branch
	•	git.merge_preview

FS
	•	fs.read
	•	fs.write
	•	fs.list
	•	fs.snapshot
	•	fs.rollback

Logs
	•	logs.tail
	•	logs.search
	•	logs.errors

Knowledge
	•	knowledge.search_local
	•	knowledge.search_docs

⸻

3.11 Output proxy blueprint

Pipeline :
	1.	commande réelle exécutée
	2.	stdout/stderr capturés
	3.	classification commande
	4.	application stratégie de compression
	5.	production de :
	•	résumé LLM
	•	sortie structurée
	•	log brut
	•	métadonnées

Exemple de structure

{
  "command": "pytest -q",
  "exit_code": 1,
  "duration_ms": 1820,
  "strategy": "failures_only",
  "summary": "2 tests échoués sur 53",
  "compact_stdout": "...",
  "compact_stderr": "",
  "raw_stdout_path": "...",
  "raw_stderr_path": "...",
  "files_detected": ["tests/test_auth.py", "src/auth.py"],
  "suggested_next_actions": ["open failing test", "show diff on auth module"]
}


⸻

3.12 Features “premium” du runtime

A. Risk engine

Scoring des actions :
	•	low
	•	medium
	•	high
	•	critical

B. Smart retries
	•	retry with narrower scope
	•	retry with verbose mode
	•	fallback command

C. Parallelization
	•	tests parallèles
	•	search parallèle
	•	multiple file reads

D. Resource governance
	•	max CPU
	•	max RAM
	•	max files touched
	•	max lines output
	•	max command count per run

E. Event hooks
	•	before_exec
	•	after_exec
	•	on_error
	•	on_diff
	•	on_merge_conflict

F. Self-healing
	•	détecter échec répétitif
	•	proposer rollback
	•	relancer stratégie alternative

⸻

3.13 Git merge assistant blueprint

Quand l’agent veut merger :
	1.	vérifier policy
	2.	vérifier état dirty
	3.	calculer merge preview
	4.	détecter conflits
	5.	résumer fichiers affectés
	6.	si conflit :
	•	extraire hunks
	•	proposer résolution
	•	demander approbation
	7.	appliquer merge ou proposer PR locale

⸻

3.14 Runtime observability

Métriques :
	•	nombre de commandes
	•	commandes refusées
	•	latence moyenne
	•	ratio sortie brute / compacte
	•	fichiers modifiés
	•	taux d’échec tests
	•	retries
	•	approvals demandées
	•	resource usage

⸻

3.15 Tables SQLite runtime
	•	runtime_sessions
	•	runtime_session_state
	•	runtime_commands
	•	runtime_command_outputs
	•	runtime_artifacts
	•	runtime_fs_events
	•	runtime_git_events
	•	runtime_policy_decisions
	•	runtime_snapshots
	•	runtime_risk_events

⸻

4. Mapping CLI ↔ Runtime

Le CLI pilote le runtime.

Exemples

Lancer un agent code

agentd run start --agent coder --runtime worktree --policy dev-safe

Ouvrir une session manuelle

agentd runtime start --repo . --mode worktree --policy dev-safe

Voir le diff

agentd git diff <session_id>

Expliquer le dernier run

agentd run explain <run_id>

Rejouer une session

agentd debug replay <run_id>


⸻

5. Priorisation produit

⸻

Phase 1 — MVP solide

CLI
	•	agent
	•	run
	•	plan
	•	runtime
	•	logs
	•	git
	•	schedule
	•	policy

Runtime
	•	session
	•	exec command
	•	permissions simples
	•	file access guard
	•	worktree mode
	•	git status/diff/commit
	•	logs JSONL
	•	output compression basique
	•	rollback simple

⸻

Phase 2 — Différenciation forte

CLI
	•	memory
	•	knowledge
	•	debug
	•	eval
	•	doctor

Runtime
	•	RTK-like proxy avancé
	•	smart file reading
	•	runtime memory
	•	merge preview
	•	risk scoring
	•	structured artifacts
	•	approvals
	•	replay

⸻

Phase 3 — Plateforme agentique complète

CLI
	•	plugins
	•	event triggers
	•	TUI
	•	template marketplace
	•	team mode

Runtime
	•	container mode
	•	cognitive runtime
	•	anomaly detection
	•	self-healing
	•	strategy switching
	•	multi-agent handoff
	•	advanced merge assistant

⸻

6. Features signature que je te conseille absolument

Si tu veux que agentd ait une vraie identité forte, je choisirais ces 8 là :

Côté CLI
	•	plans adaptatifs
	•	scheduler natif
	•	explain mode
	•	replay
	•	memory
	•	eval
	•	policy profiles
	•	knowledge search

Côté runtime
	•	worktree par session
	•	permission engine fin
	•	output proxy intelligent type RTK++
	•	logs/artifacts complets
	•	git diff/commit/merge preview
	•	file access guard
	•	risk scoring
	•	runtime memory

⸻

7. Vision finale du produit

Le produit final peut ressembler à ça :

CLI = control plane

Il :
	•	configure
	•	orchestre
	•	planifie
	•	observe
	•	explique

Runtime = execution plane

Il :
	•	exécute
	•	protège
	•	résume
	•	versionne
	•	journalise

Knowledge/memory = cognition plane

Il :
	•	retrouve
	•	se souvient
	•	cite
	•	apprend

⸻

8. Résumé ultra net

Le CLI doit devenir
	•	un orchestrateur d’agents
	•	un moteur de plans
	•	un scheduler
	•	un centre d’audit
	•	un gestionnaire de mémoire/knowledge
	•	un outil de debug et d’évaluation

Le runtime doit devenir
	•	un environnement d’exécution agentique sécurisé
	•	Git-aware
	•	file-aware
	•	policy-driven
	•	observable
	•	capable de compresser intelligemment les sorties
	•	capable de replay, rollback, merge assisté et audit complet

⸻

9. Recommandation finale

La meilleure stratégie pour toi :
	1.	faire du runtime intégré la feature signature
	2.	faire du CLI la couche d’orchestration propre et lisible
	3.	ajouter ensuite mémoire + knowledge + eval
	4.	pousser enfin vers autonomie et plugins

Le plus différenciant, franchement, c’est ce trio :
	•	runtime Git-aware
	•	permissions fines
	•	proxy de sortie intelligent type RTK intégré

Si tu veux, je peux te transformer ce blueprint en roadmap produit détaillée sur 3 mois, ou en spec technique Rust avec modules, traits, structs et tables SQLite.