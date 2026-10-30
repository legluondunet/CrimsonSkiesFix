# Crimson Skies 1.0.2.0 — Prototype natif SafeDisc v0.9

**Banc de recherche expérimental, non validé en jeu.** Aucun fichier propriétaire n'est fourni dans cette archive. Python 3.10+ fonctionne sous Linux/Windows, sans exécuter Windows 98 ni la DLL SafeDisc.

## Deux modes strictement séparés

1. **Mode normal :** reconstruit les sections `.text` et `.data` (F1/F2 + TEA), les 245 appels IAT, les tables de DLL et les 136 noms chiffrés. Produit un candidat PE32. SHA-256 exact de ce candidat : `a39570f6a3d33496b03757fca15d0153e37a91f215e93b6c117eae98d088eabc`. Ce mode n'ajoute pas de métadonnées historiques inutiles.
2. **Mode `--historical-output-profile` :** ajoute uniquement des changements d'en-tête, nom de DLL et remplissage propres à la sortie *observée* d'unSafeDisc 1.5.5 sur ce **jeu/version précise**. SHA-256 final : `43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75`. Egalité de **2 580 480 / 2 580 480 octets** avec la référence. Ce mode n'est pas un algorithme de déchiffrement générique.

La référence `testme.exe` n'est **jamais** lue pour produire le candidat. Si `--reference` est présent, il sert uniquement à effectuer une comparaison après la reconstruction, avant écriture.

## Utilisation

```
python3 crimson_unwrap_candidate.py \
  --icd /jeux/CRIMSON.ICD \
  --snapshot /captures/CS98-DLL-00.BIN \
  --output /tmp/CRIMSON-CANDIDAT.EXE
```

Pour reproduire *bit à bit* la sortie connue d'unSafeDisc 1.5.5 :

```
python3 crimson_unwrap_candidate.py \
  --icd /jeux/CRIMSON.ICD \
  --snapshot /captures/CS98-DLL-00.BIN \
  --loader /jeux/CRIMSON.EXE \
  --historical-output-profile \
  --output /tmp/CRIMSON-HISTORIQUE.EXE
```

Ajouter facultativement `--reference /jeux/testme.exe` pour une comparaison **en lecture seule** à la référence historique déjà fournie par le propriétaire.

Tous les fichiers d'entrée nécessaires sont vérifiés par SHA-256. Les sorties existantes sont refusées ; les fichiers protégés ne sont pas modifiés. Une empreinte de sortie exacte est exigée avant toute création du fichier.

## Recherche forensique : provenance des 61 octets

| Zone | Nombre | Avant → profil historique | Statut |
| --- | ---: | --- | --- |
| `0xF6..0xF7` | 2 | Complète `0xAD2B0000` à `e_lfanew-4`, valeur également stockée dans les 21 descripteurs d'importation reconstruits | **Corrélation réelle** ; motif non expliqué dans le code historique |
| `0x100..0x103` | 4 | Horodatage PE original → `1911` | Constante observée dans la sortie historique, provenance de l'outil non prouvée |
| `0x300..0x32F` | 48 | Signature historique ASCII unSafeDisc dans le *header slack* | Signature de l'outil, documentée publiquement ; non liée au chiffrement |
| `0x2452E3` et `0x2452E6` | 2 | `KERNEL32.dll` → `KeRNeL32.dll` | Casse observée dans la sortie historique ; non nécessaire aux importations Windows |
| `0x202FFD..0x202FFF` | 3 | `05 43 13` = taille de `CRIMSON.EXE` (`344851 = 0x054313`) en big-endian sur trois octets | Corrélation exacte, **algorithme historique non prouvé** ; en dehors de VirtualSize de `.text` |
| `0x275FFD..0x275FFE` | 2 | `27 60` = les deux premiers octets de la taille de sortie `0x276000` encodée sur 3 octets big-endian | Corrélation exacte, **algorithme historique non prouvé** ; en dehors de VirtualSize de `.reloc` |

La commande v0.9 n'obtient **pas** sa sortie identique en copiant les 61 octets depuis `testme.exe` : les valeurs sont calculées d'après le fichier d'entrée, le PE reconstruit, ou des constantes de style observées et explicitement documentées. Il serait cependant incorrect de présenter les trois valeurs de style (`1911`, auteur, casse) comme **redécouvertes par désassemblage** de la version originale d'unSafeDisc ; l'algorithme historique de post-traitement n'a pas encore été reconstitué intégralement.

## Vérifications locales du 28/10/2026

- D'origine : `CRIMSON.ICD` et `CRIMSON.EXE` SHA-256 connus et exacts ; capture Win98 `CS98-DLL-00.BIN` hash exact.
- Traitement cryptographique : `.text` **1 855 489 / 1 855 489** et `.data` **168 072 / 168 072** octets non-HIGHLOW identiques à la référence ; **440 422** mots F1/F2 confirmés pour `.text`.
- Avec le profil historique : sortie **2 580 480 octets**, SHA-256 **identique** à `testme.exe`, `cmp` **sans différence**.
- **Non vérifié :** lancement du jeu, cinématiques, progression de toutes les missions et compatibilité avec les correctifs résolution/FOV et Wine.

La PR de recherche GitHub reste en brouillon : https://github.com/legluondunet/crimson-skies-resolution-patcher/pull/5
