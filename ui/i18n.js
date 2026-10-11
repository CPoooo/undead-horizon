// Localization: string tables keyed by the English text. Static UI text is
// translated in place (leaf elements whose whole text matches a key); code can
// call window.t('English text') for dynamic strings. English is the source.
(() => {
  'use strict';
  const TABLES = {
    es: {
      'Mode': 'Modo', 'Difficulty': 'Dificultad', 'Survivor name': 'Nombre del superviviente',
      '▶ PLAY': '▶ JUGAR', '🏠 Enter Home Base': '🏠 Entrar a la Base', '☀️ Daily Run': '☀️ Partida diaria',
      '🏆 Leaderboard': '🏆 Clasificación', '🛒 Shop': '🛒 Tienda', '🎒 Locker': '🎒 Taquilla', '🏅 Career': '🏅 Carrera',
      '👥 Friends': '👥 Amigos', '⚙️ Settings': '⚙️ Ajustes', 'Sound': 'Sonido', 'Music': 'Música',
      'Rookie': 'Novato', 'Survivor': 'Superviviente', 'Veteran': 'Veterano', 'Nightmare': 'Pesadilla',
      '♾️ Infinite': '♾️ Infinito', '📖 Campaign': '📖 Campaña', '🧟 Outbreak': '🧟 Brote', '🏰 Siege': '🏰 Asedio',
      'Chapter': 'Capítulo', 'Map': 'Mapa', 'Siege map': 'Mapa de asedio', 'Controls': 'Controles', 'Key bindings': 'Teclas',
      'Resume': 'Continuar', '💾 Save game': '💾 Guardar', 'Save & quit to menu': 'Guardar y salir', 'Abandon run': 'Abandonar partida',
      'YOU DIED': 'HAS MUERTO', 'VICTORY': 'VICTORIA', 'Wave reached': 'Oleada', 'Level': 'Nivel', 'Kills': 'Bajas',
      'Survived': 'Sobreviviste', 'Score': 'Puntos', '↻ PLAY AGAIN': '↻ JUGAR DE NUEVO', 'Main menu': 'Menú principal',
      'Settings': 'Ajustes', 'Language': 'Idioma', 'Say': 'Decir', 'Reset': 'Restablecer', 'Change': 'Cambiar',
      'Play': 'Jugar', 'Board': 'Tabla', 'Shop': 'Tienda', 'Locker': 'Taquilla', 'Career': 'Carrera', 'More': 'Más', 'Friends': 'Amigos',
    },
  };
  const LANGS = [['en', 'English'], ['es', 'Español']];
  let lang = 'en';
  const t = (s) => (TABLES[lang] && TABLES[lang][s]) || s;
  function apply(root = document.body) {
    if (!root) return;
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
    const nodes = [];
    while (walker.nextNode()) nodes.push(walker.currentNode);
    for (const n of nodes) {
      const el = n.parentElement;
      if (!el || el.closest('script,style,#career,#locker,#shop-items,#lb-menu')) continue;
      const src = n.__en ?? n.textContent.trim();
      if (!src) continue;
      if (n.__en === undefined) {
        if (!(src in (TABLES.es || {}))) continue; // only known strings
        n.__en = src;
      }
      const out = t(n.__en);
      if (n.textContent.trim() !== out) n.textContent = n.textContent.replace(n.textContent.trim(), out);
    }
    document.documentElement.lang = lang;
  }
  function setLang(l) {
    lang = TABLES[l] || l === 'en' ? l : 'en';
    apply();
  }
  // re-translate as the UI re-renders
  let pending = 0;
  new MutationObserver(() => {
    if (lang === 'en' || pending) return;
    pending = requestAnimationFrame(() => { pending = 0; apply(); });
  }).observe(document.documentElement, { childList: true, subtree: true });
  window.t = t;
  window.UHI18n = { setLang, apply, LANGS, get lang() { return lang; } };
})();
