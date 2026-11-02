//! Intelligence comportementale : machine à états hybride (états explicites, décisions pondérées à
//! l'intérieur de chaque état) inspirée des moteurs « combat / non-combat / dead » de mod-playerbots, sans
//! en reprendre la mécanique de stratégies nommées : ArchLord n'a ni talents ni rotations de dizaines de
//! sorts, un arbre de décision direct suffit et reste lisible.

pub mod brain;
pub mod state;
