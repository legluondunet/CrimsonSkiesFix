# CrimsonSkiesFix — CD français vers 1.02, reconstruction native (TEST UNIQUEMENT)

**Compilation expérimentale Windows x86. Pas une version publique stable.**

Cette variante `native-fr-cd` intègre directement dans le même
`CrimsonSkiesFix.exe` le moteur GPL Rust de la PR #5 de `unsafedisk`,
basé sur la reconstruction 1.02 de sa PR #2. Aucune copie du jeu,
aucun correctif Microsoft, aucun exécutable commercial ou DLL SafeDisc
n'est embarqué. Aucun téléchargement n'est effectué à l'installation.

## Pour effectuer une installation française fraîche

1. Créer un dossier de test **distinct** de toute installation existante.
2. Placer uniquement `CrimsonSkiesFix.exe` et **l'image de votre CD français
   original** : `.iso` ou `.cue` accompagné de son `.bin`/`.img`.
3. Lancer `CrimsonSkiesFix.exe --version` et vérifier la mention
   `NATIVE FR 1.02 EXPERIMENTAL`.
4. Lancer `CrimsonSkiesFix.exe --install-only`.
5. Vérifier la présence des deux répertoires `game/` et `state/`, puis
   lancer `CrimsonSkiesFix.exe --verify`.
6. Lancer ensuite `CrimsonSkiesFix.exe` pour démarrer le jeu.

Aucun `crmsk102.exe` ni `CRIMSON.EXE` déchiffré à rechercher ou à
copier : **le CD FR d'origine fournit déjà les fichiers 1.02 nécessaires**.

Le CD est reconnu par les SHA-256 complets de ses fichiers internes, pas
par le nom ISO. L'installeur revalide les originaux, les données GOS, les
chapitres CAB et le `CRIMPTCH.ROF` 797 octets présent sur le CD français.
Le moteur reconstitue le binaire exécutable avec contrôle SHA-256 strict
`43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75`.
L'exécutable CD protégé est conservé sous
`game/CRIMSON.EXE.protected.bak` ; le fichier temporaire de
reconstruction est supprimé après activation.

La structure conserve `game/` pour le jeu, `fix.dll` embarqué dans le
lanceur, `docs/` et `state/` pour les reçus. Les journaux
`CrimsonSkiesFix_installer.log` et `CrimsonSkiesFix.log` restent à côté
du lanceur, non cumulatifs. L'installateur ne migre pas automatiquement
les anciennes arborescences.

## Limitations et sécurité

- **Les CD US/Europe et DE 1.00 ne sont PAS convertis automatiquement
  en 1.02.** Leur parcours historique continue d'exiger l'update officiel
  authentifié et l'EXE 1.02 de référence.
- Toute source non authentifiée, tout fichier temporaire déjà présent ou
  tout résultat EXE différent du SHA-256 attendu provoque un refus, sans
  lancer l'EXE protégé.
- Une reconstruction/réparation conserve les sauvegardes `SavedGames/`
  et `Planes/` et l'ancienne installation lors des opérations de rollback.
- Les tests synthétiques CI Windows x86 et le moteur Rust sont vérifiés,
  **mais l'installation complète, le lancement et le gameplay sur CD FR
  original doivent encore être validés sous Windows et Wine/Proton**.
- Cette compilation est une expérience d'interopérabilité sur les médias
  du propriétaire. La diffusion d'outils de contournement de protection
  demande une revue juridique adaptée avant toute publication.

Pour signaler un problème, joindre le journal installateur, puis le
journal runtime si le jeu a démarré. Ne jamais publier de clés privées,
de fichiers propriétaires ni le CD du jeu.
