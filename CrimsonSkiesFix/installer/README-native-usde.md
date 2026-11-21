# CrimsonSkiesFix — test US/Europe et allemand avec mise à jour 1.02 locale

**Build expérimental Win32 x86, non destiné à une release publique.**

Ce test apporte une première automatisation pour les CD originaux **US/Europe** et **allemands**. Le launcher vérifie l'image de CD et une copie **déjà possédée** du correctif officiel Microsoft 1.02, mais **n'exige plus d'exécutable déchiffré**.

L'installateur ne télécharge et n'exécute jamais le correctif Microsoft. Il lit le CAB intégré **en mémoire** après vérification de la taille, du SHA-256 et de sa structure ; les sources protégées 1.02 sont traitées par la bibliothèque Rust `unsafedisk` dans le même processus. Seul l'EXE 1.02 final est produit temporairement, puis activé via la transaction existante et sa sauvegarde du chargeur protégé CD. Aucun fichier propriétaire n'est embarqué.

**Important :** la voie **CD US/DE seul**, sans correctif, reste un chantier distinct et non résolu. Cette compilation sert d'étape fonctionnelle, et ne représente pas encore le critère final de l'issue #46.

## Test d'installation neuve

1. Créer un répertoire de test **distinct** de l'installation française validée ou de toute sauvegarde existante.
2. Y placer `CrimsonSkiesFix.exe`, une image du **CD original US/Europe ou allemand** (`.iso` ou `.cue` avec `.bin`/`.img`), ainsi que votre **copie authentique** du correctif Microsoft 1.02. Le nom du correctif est libre : seul son contenu SHA-256 est utilisé.
3. **Ne pas ajouter d'EXE déchiffré**. La reconstruction est réalisée par l'installateur.
4. Exécuter `CrimsonSkiesFix.exe --version` et vérifier `NATIVE US/DE LOCAL UPDATE 1.02 EXPERIMENTAL`.
5. Exécuter `CrimsonSkiesFix.exe --install-only`, puis `CrimsonSkiesFix.exe --verify` ; ensuite lancer `CrimsonSkiesFix.exe` pour jouer.
6. Conserver `CrimsonSkiesFix_installer.log` et `CrimsonSkiesFix.log` en cas de problème.

Le correctif reconnu a une taille de 2 315 728 octets et une empreinte
SHA-256 `8e1c04f3e32c318a58ef3e2bbd8f13b9894ff055d4492f80ee14ce53851616af`.
Les données exécutables finales sont validées sur le SHA-256 officiel de notre référence 1.02 `43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75`.

La source CD protégée est conservée en `game/CRIMSON.EXE.protected.bak`. Les données sauvegardées `SavedGames/` et `Planes/` et le mécanisme de rollback restent pris en charge. Le jeu et `fix.dll` résident dans `game/`, avec `state/`, `docs/` et les logs à côté du lanceur.

Le **CD français original** reste utilisable sans correctif externe, comme dans la branche native FR précédemment validée.

## Limites

- Cette compilation nécessite encore le correctif 1.02 **local** pour US/DE. Aucun téléchargement, aucune redistribution, aucun correctif privé incorporé.
- Les essais CI vérifient la compilation Windows, le refus de CD manquant et le parcours synthétique. L'installation et le gameplay **réels** de ces éditions via cette nouvelle voie demandent encore une validation sous Wine/Windows.
- Les binaires non reconnus et les résultats différents du SHA-256 attendu sont rejetés.
- La PR est expérimentale. Conserver la validation FR existante et effectuer une revue technique et juridique avant toute fusion/release.
