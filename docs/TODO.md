# agentd - Plan A a Z

Ce document definit un plan complet, incremental et verifiable pour faire de `agentd` un runtime agentique controle, sans vendor lock-in.

## Principes directeurs

- Ports/adapters stricts: separer policies, execution, git, output, stockage.
- Secure by default: deny by default, permissions explicites, traces obligatoires.
- Reproductible: sessions runtime identifiees, artifacts persistes, replay possible.
- Evolutif: MVP livrable vite, puis extension progressive.

## Phase A - Stabiliser la base

Objectif: partir d'un tronc propre avant d'ajouter le nouveau runtime.

Taches:

- Verrouiller les gates CI locales: `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`.
- Ajouter une section "Definition of Done" commune dans ce fichier.
- Documenter les conventions de logs et de nommage des events runtime.

Livrables:

- Build vert local.
- Convention events/logs validee.

## Phase B - Modele RuntimeSession

Objectif: introduire le conteneur d'execution d'une session agent.

Taches:

- Creer `src/domain/runtime_session.rs`:
  - `RuntimeSessionId`
  - `RuntimeMode` (`local`, `worktree`, `sandbox`)
  - `RuntimeSession`
- Ajouter metadata session:
  - `workspace_dir`, `repo_root`, `base_commit`, `branch_name`
  - `permissions_profile`, `env_profile`, `log_dir`
- Ajouter tests unitaires du modele (serde + defaults).

Livrables:

- Nouvelles structures domain compilees et testees.

## Phase C - Policy Engine par capacites

Objectif: encoder les permissions metier (pas uniquement des noms de commandes).

Taches:

- Creer `src/domain/capability.rs`:
  - `Capability` (`ReadFile`, `WriteFile`, `ExecShell`, `ExecGitRead`, `ExecGitWrite`, `MergeBranch`, etc.)
  - `PolicyDecision` (`allow`, `reason`, `matched_rule`)
- Creer `src/ports/policy.rs` avec `PolicyPort`.
- Adapter local policy:
  - mapping commande -> capability
  - allowlist/denylist commandes
  - allowlist/denylist chemins
  - policy profiles (`read-only`, `dev-safe`, `repo-maintainer`, `full-trusted`)
- Ajouter tests de decisions policy.

Livrables:

- Policy evaluable avant chaque action runtime.

## Phase D - Workspace Guard

Objectif: proteger l'acces fichiers contre traversal/symlink bypass.

Taches:

- Creer `src/ports/workspace_guard.rs`.
- Implementer guard local:
  - canonicalisation des chemins
  - resolution symlinks
  - blocage `..` et sorties hors workspace
  - split read/write permissions
  - blocage chemins sensibles (`.git`, `.env`, secrets)
- Ajouter tests d'attaque path traversal et symlink.

Livrables:

- Verifications read/write robustes et testees.

## Phase E - Runtime Executor central

Objectif: interdire l'execution directe des commandes depuis les providers.

Taches:

- Creer `src/app/runtime_executor.rs`.
- Exposer API metier:
  - `exec(command, args, options)`
  - `read_file(path, range)`
  - `write_file(path, content)`
  - `apply_patch(diff)`
  - `git_status()`, `git_diff()`, `git_commit()`
- Orchestration obligatoire:
  - `PolicyPort` -> `WorkspaceGuard` -> `RuntimePort`
- Integrer limites ressources (`timeout`, volume sortie max).

Livrables:

- Point d'entree unique pour execution/outils.

## Phase F - Output Adapter (proxy interne type RTK)

Objectif: stocker brut + retourner compact au modele.

Taches:

- Creer `src/ports/output_adapter.rs`.
- Creer `src/adapters/output/compact_adapter.rs`.
- Definir `CompactOutput`:
  - `summary`, `compact_stdout`, `compact_stderr`
  - `raw_log_path`, `strategy`, `duration_ms`, `exit_code`
- Implementer strategies MVP:
  - `git_status_summary`
  - `git_diff_stats`
  - `test_failures_only`
  - `dedupe_repeated_lines`
  - `head_tail_large_output`
- Ajouter tee systematique des logs bruts sur disque.

Livrables:

- Sorties LLM-friendly sans perdre le brut.

## Phase G - Git Controller de haut niveau

Objectif: fiabiliser workflow git via operations explicites.

Taches:

- Creer `src/ports/git_controller.rs`.
- Implementer adapter git local:
  - `snapshot_base`
  - `status`
  - `diff_working`
  - `diff_against_base`
  - `apply_patch`
  - `commit`
  - `create_branch`
  - `merge`
  - `abort_merge`
- Integrer policy merge (`manual|assisted|auto`), default `manual`.
- Ajouter tests integration git (conflicts, dry-run, abort).

Livrables:

- Couche git controllee, auditable et testee.

## Phase H - Runtime Session Manager

Objectif: cycle de vie complet des sessions runtime.

Taches:

- Creer `src/ports/runtime_session.rs`.
- Creer `src/app/runtime_session_manager.rs`:
  - create/start/close session
  - provision mode `worktree` par defaut
  - cleanup des worktrees/session dirs
- Branch naming deterministe (`agentd/sess_<id>`).
- Hooks de recover au redemarrage.

Livrables:

- Sessions runtime pilotables de bout en bout.

## Phase I - Persistence runtime (SQLite)

Objectif: audit complet + replay possible.

Taches:

- Ajouter tables:
  - `runtime_sessions`
  - `runtime_events`
  - `runtime_artifacts`
- Persister chaque action:
  - contexte, decision policy, sortie compacte, pointeur log brut.
- Ajouter requetes de listing/filtrage par `session_id`.

Livrables:

- Historique runtime exploitable et filtrable.

## Phase J - Surface CLI runtime

Objectif: exposer la nouvelle couche runtime dans le CLI.

Taches:

- Ajouter commandes:
  - `runtime-session-start`
  - `runtime-session-status`
  - `runtime-session-stop`
  - `runtime-exec`
  - `runtime-events`
  - `runtime-artifacts`
- Ajouter flags:
  - `--permissions-profile`
  - `--runtime-mode`
  - `--report-json`
- Unifier erreurs structurees (categories + causes).

Livrables:

- Runtime operable depuis CLI avec output scriptable.

## Phase K - Multi-profils CLI provider

Objectif: supporter plusieurs profils `cli` simultanes sans vendor lock-in.

Taches:

- Etendre config:
  - `[providers.cli_profiles.<name>]`
- Resolution provider:
  - `--provider cli:<profile>`
  - fallback sur profil par defaut `cli:default`
- Compat backward:
  - conserver `[providers.cli]` comme alias `default`
- Tests integration:
  - profils multiples dans un meme plan
  - override via env par execution

Livrables:

- Multi-provider CLI natif et propre.

## Phase L - Hardening final

Objectif: securiser et fiabiliser avant generalisation.

Taches:

- Ajouter limites runtime:
  - max stdout/stderr
  - max fichiers modifies
  - max duree session
- Ajouter mode preview pour actions risquees.
- Ajouter rollback session:
  - reset worktree
  - revert dernier patch/commit session
- Ajouter bench basic sur output adapter.

Livrables:

- Runtime robuste pret pour usage intensif.

## Phase M - Documentation et examples

Objectif: rendre la feature adoptable vite.

Taches:

- Mettre a jour `README.md` (runtime session + profils + policy).
- Ajouter exemples:
  - mode `worktree`
  - policy `read-only`
  - workflow commit + proposal merge
- Ajouter guide "debug runtime".

Livrables:

- Documentation complete orientee usage.

## Definition of Done (globale)

Une phase est complete si:

- les tests associes passent,
- `clippy -D warnings` est vert,
- la doc de la phase est a jour,
- les events runtime sont traces et auditables.

## Ordre recommande d'execution

1. A
2. B
3. C
4. D
5. E
6. F
7. G
8. H
9. I
10. J
11. K
12. L
13. M

## Risques principaux

- Evasion de policy via chemins/symlinks.
- Volume logs trop important sans compactage/tee.
- Complexite git (merge/conflicts) sans garde-fous.
- Regression UX si output non structure.

## Criteres de succes produit

- Sessions runtime isolees et reproductibles.
- 100% des actions critiques passent par policy.
- Sortie compacte utile + logs bruts recuperables.
- Workflow git agentique controllable (diff/commit/propose-merge).
- Multi-profils CLI disponibles sans lock-in fournisseur.