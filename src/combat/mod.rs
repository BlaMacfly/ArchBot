//! Combat : choix de la cible, survie, stratégies par classe. Les coups eux-mêmes, la portée effective, les
//! dégâts et l'expérience sont calculés par le serveur ; le moteur décide seulement qui attaquer, quand se
//! placer, quelle compétence lancer et quand se protéger.

pub mod classes;
pub mod survival;
pub mod targeting;
