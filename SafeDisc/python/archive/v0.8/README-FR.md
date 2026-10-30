# Crimson Skies 1.0.2.0 — Prototype de reconstruction native SafeDisc v0.8

**Expérimental / non validé en jeu.** Cet outil Python indépendant reconstruit un exécutable candidat pour le jeu PC original Crimson Skies 1.0.2.0, SafeDisc 1.50.20. Il ne lance aucun logiciel Windows ni DLL protégée. Il ne modifie jamais l'ICD ou la capture source.

L'objectif final reste le portage **Rust** dans le projet `crimson-skies-resolution-patcher`. Cette version est un banc de validation temporaire.

## Contenu

- `crimson_unwrap_candidate.py` : reconstruction en lecture seule des entrées et création d'un *nouveau* fichier EXE après contrôle d'empreinte.
- `analyze_modifiers.py` : décodeur borné des fonctions arithmétiques F1/F2 du cliché Win98.
- `permutation_metadata.txt` : deux permutations de pointeurs, propres à cette version du jeu, dérivées de la comparaison avec la référence historique. Ne contient aucun code du jeu.
- `import_site_rvas.txt` : 245 emplacements de corrections IAT, propres à ce binaire exact.

**Aucune donnée originale du jeu, aucun binaire SafeDisc et aucun cliché mémoire ne sont distribués dans l'archive.**

## Utilisation (Linux, Python 3.10+)

Conserver les quatre fichiers du prototype dans le même dossier, puis lancer :

```bash
python3 crimson_unwrap_candidate.py \
  --icd /chemin/vers/CRIMSON.ICD \
  --snapshot /chemin/vers/CS98-DLL-00.BIN \
  --output /chemin/vers/CRIMSON-CANDIDAT.EXE
```

Une comparaison indépendante optionnelle avec le `testme.exe` original et *non modifié* est disponible :

```bash
python3 crimson_unwrap_candidate.py \
  --icd /chemin/vers/CRIMSON.ICD \
  --snapshot /chemin/vers/CS98-DLL-00.BIN \
  --reference /chemin/vers/testme.exe \
  --output /chemin/vers/CRIMSON-CANDIDAT.EXE
```

L'exécutable candidat est créé **seulement** si toutes les vérifications SHA-256 exactes et géométriques passent. Une sortie existante n'est jamais écrasée ; choisir un nouveau nom si besoin.

## Résultats réels de la validation locale du 28 octobre 2026

- `CRIMSON.EXE`, `CRIMSON.ICD`, `dplayerx.dll`, `testme.exe` et `CS98-DLL-00.BIN` : empreintes originales exactes vérifiées.
- Fonctions F1/F2 : 440 422/440 422 mots de `.text` conformes ; chaîne F1/F2 → TEA : 1 855 489/1 855 489 octets non-HIGHLOW `.text` conformes.
- `.data` : 168 072/168 072 octets non-HIGHLOW conformes.
- Correction ciblée de 245 appels IAT ; récupération de 136 noms d'API par XOR cumulatif, indice initial 0xDF ; restauration des tables d'importation KERNEL32/USER32.
- Fichier candidat de **2 580 480 octets**, SHA-256 `a39570f6a3d33496b03757fca15d0153e37a91f215e93b6c117eae98d088eabc`.
- **2 580 419 octets identiques** à `testme.exe` produit historiquement sous Win98. Les 61 octets restants se répartissent : 54 marqueurs/en-têtes historiques, 2 variations de casse dans le nom de KERNEL32, 3 octets de remplissage `.text`, 2 octets de remplissage `.reloc`.
- Répertoire PE32 cohérent : 6 sections, 21 bibliothèques importées, 597 importations valides (392 par nom et 205 par ordinal).

**Important** : les permutations IAT et les listes de sites de correction sont spécifiques au *hash exact* de Crimson Skies 1.0.2.0 ; elles ne constituent pas un déchiffreur SafeDisc généraliste. Le fonctionnement du jeu, les cinématiques et la compatibilité avec le patch résolution/FOV restent à tester sur une installation réelle. Ne pas remplacer l'exécutable d'origine sans copie de sauvegarde.

Projet : https://github.com/legluondunet/crimson-skies-resolution-patcher/pull/5
