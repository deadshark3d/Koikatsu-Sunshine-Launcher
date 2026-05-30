// main.js — frontend: DOM, eventos, IPC con Tauri

const { invoke } = window.__TAURI__.core;

let appWindow = null;
try       { appWindow = window.__TAURI__.window.getCurrentWindow(); }
catch (_) { try { appWindow = window.__TAURI__.window.appWindow; } catch (_) {} }

document.addEventListener('contextmenu', e => e.preventDefault());

// ============================================================
// GLASS — displacement map + aberración cromática
// ============================================================

/**
 * Genera el SVG del displacement map para el efecto glass.
 * @param {object} opts  width, height, radius, border, lightness, alpha, blur, blend
 */
function buildDisplacementMap({ width, height, radius,
  border = 0.08, lightness = 50, alpha = 0.93, blur = 10, blend = 'difference' }) {

  const b   = Math.min(width, height) * (border * 0.5);
  const uid = `${width}x${height}`;

  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${width} ${height}">
    <defs>
      <linearGradient id="r${uid}" x1="100%" y1="0%" x2="0%" y2="0%">
        <stop offset="0%" stop-color="#000"/>
        <stop offset="100%" stop-color="red"/>
      </linearGradient>
      <linearGradient id="b${uid}" x1="0%" y1="0%" x2="0%" y2="100%">
        <stop offset="0%" stop-color="#000"/>
        <stop offset="100%" stop-color="blue"/>
      </linearGradient>
    </defs>
    <rect width="${width}" height="${height}" fill="black"/>
    <rect width="${width}" height="${height}" rx="${radius}" fill="url(#r${uid})"/>
    <rect width="${width}" height="${height}" rx="${radius}"
          fill="url(#b${uid})" style="mix-blend-mode:${blend}"/>
    <rect x="${b}" y="${b}"
          width="${Math.max(0, width  - b * 2)}"
          height="${Math.max(0, height - b * 2)}"
          rx="${Math.max(0, radius - b)}"
          fill="hsl(0 0% ${lightness}% / ${alpha})"
          style="filter:blur(${blur}px)"/>
  </svg>`;
}

function svgToDataUri(svgStr) {
  return `data:image/svg+xml,${encodeURIComponent(svgStr)}`;
}

// Parámetros globales del efecto glass (ajustables en runtime si se expone un panel avanzado)
const GS = {
  border:     0.07,
  lightness:  50,
  alpha:      0.93,
  blur:       11,
  blend:      'difference',
  scale:      -180,
  chX:        'R',
  chY:        'B',
  r:          0,
  g:          10,
  b:          20,
  outputBlur: 0.2,
  saturation: 1.5,
};

// borderMul compensa el tamaño de cada elemento para que el borde del efecto sea proporcional
const FILTERS = [
  { feImageId: 'gm-btn',   rId: 'btn-r',   gId: 'btn-g',   bId: 'btn-b',   blurId: 'btn-blur',   width: 250, height: 70,  radius: 13.5, borderMul: 1.86 },
  { feImageId: 'gm-panel', rId: 'panel-r', gId: 'panel-g', bId: 'panel-b', blurId: 'panel-blur', width: 500, height: 535, radius: 13.5, borderMul: 0.71 },
  { feImageId: 'gm-ctrl',  rId: 'ctrl-r',  gId: 'ctrl-g',  bId: 'ctrl-b',  blurId: 'ctrl-blur',  width: 90,  height: 40,  radius: 13.5, borderMul: 2.57 },
];

function applyGlassFilters() {
  FILTERS.forEach(({ feImageId, rId, gId, bId, blurId, width, height, radius, borderMul }) => {
    const opts = {
      width, height, radius,
      border:    GS.border * borderMul,
      lightness: GS.lightness,
      alpha:     GS.alpha,
      blur:      GS.blur,
      blend:     GS.blend,
    };

    const feImg = document.getElementById(feImageId);
    if (feImg) feImg.setAttribute('href', svgToDataUri(buildDisplacementMap(opts)));

    const setScale = (id, offset) => {
      const el = document.getElementById(id);
      if (el) el.setAttribute('scale', String(GS.scale + offset));
    };
    setScale(rId, GS.r);
    setScale(gId, GS.g);
    setScale(bId, GS.b);

    [rId, gId, bId].forEach(id => {
      const el = document.getElementById(id);
      if (!el) return;
      el.setAttribute('xChannelSelector', GS.chX);
      el.setAttribute('yChannelSelector', GS.chY);
    });

    const blurEl = document.getElementById(blurId);
    if (blurEl) blurEl.setAttribute('stdDeviation', String(GS.outputBlur));
  });

  // Actualizar variables CSS de backdrop-filter
  const launcher = document.getElementById('launcher');
  if (launcher) {
    const sat = GS.saturation;
    launcher.style.setProperty('--gf-btn',   `url(#gf-btn)   blur(4px) brightness(1.12) saturate(${sat})`);
    launcher.style.setProperty('--gf-panel', `url(#gf-panel) blur(6px) brightness(1.06) saturate(${sat})`);
    launcher.style.setProperty('--gf-ctrl',  `url(#gf-ctrl)  blur(3px) brightness(1.12) saturate(${sat})`);
  }
}


// ============================================================
// PARALLAX
// ============================================================

const bg       = document.querySelector('.bg');
const logoEl   = document.querySelector('.logo');
const STRENGTH = 14;
const LERP     = 0.07;
const SCALE_IN = 1.055; // zoom para cubrir el movimiento sin mostrar bordes

let target  = { x: 0, y: 0, s: 1 };
let current = { x: 0, y: 0, s: 1 };
let rafId   = null;

function parallaxTick() {
  current.x += (target.x - current.x) * LERP;
  current.y += (target.y - current.y) * LERP;
  current.s += (target.s - current.s) * LERP;

  bg.style.transform = `translate(${current.x.toFixed(2)}px, ${current.y.toFixed(2)}px) scale(${current.s.toFixed(4)})`;

  const running =
    Math.abs(target.x - current.x) > 0.05  ||
    Math.abs(target.y - current.y) > 0.05  ||
    Math.abs(target.s - current.s) > 0.0005;

  rafId = running ? requestAnimationFrame(parallaxTick) : null;
}

launcher.addEventListener('mousemove', e => {
  const { width, height, left, top } = launcher.getBoundingClientRect();
  target.x = -(e.clientX - left  - width  / 2) / (width  / 2) * STRENGTH;
  target.y = -(e.clientY - top   - height / 2) / (height / 2) * STRENGTH;
  target.s = SCALE_IN;
  if (!rafId) rafId = requestAnimationFrame(parallaxTick);
});

launcher.addEventListener('mouseleave', () => {
  target.x = 0; target.y = 0; target.s = 1;
  if (!rafId) rafId = requestAnimationFrame(parallaxTick);
});


// ============================================================
// CUSTOM SELECT
// ============================================================

class CustomSelect {
  constructor(id) {
    this.wrap    = document.getElementById(id);
    this.trigger = this.wrap.querySelector('.cs-trigger');
    this.label   = this.wrap.querySelector('.cs-label');
    this.opts    = this.wrap.querySelectorAll('.cs-opt');
    this.wrap._cs = this;

    this.trigger.addEventListener('click', e => { e.stopPropagation(); this.toggle(); });
    this.opts.forEach(opt => {
      opt.addEventListener('click', e => { e.stopPropagation(); this._select(opt); });
    });
  }

  get value() { return this.wrap.dataset.value ?? ''; }
  get code()  { return this.wrap.dataset.code  ?? ''; }

  toggle() {
    if (this.wrap.classList.contains('open')) this.close();
    else {
      document.querySelectorAll('.cs-wrap.open').forEach(w => w._cs?.close());
      this.open();
    }
  }
  open()  { this.wrap.classList.add('open'); }
  close() { this.wrap.classList.remove('open'); }

  _select(opt) {
    this.opts.forEach(o => o.classList.remove('selected'));
    opt.classList.add('selected');
    this.label.textContent  = opt.textContent.trim();
    this.wrap.dataset.value = opt.dataset.value;
    if (opt.dataset.code) this.wrap.dataset.code = opt.dataset.code;
    this.close();
    this.wrap.dispatchEvent(new CustomEvent('cs:change', {
      bubbles: true,
      detail: { value: this.value, code: this.code },
    }));
  }

  setValue(value)  { const opt = [...this.opts].find(o => o.dataset.value === String(value));  if (opt) this._select(opt); }
  setByCode(code)  { const opt = [...this.opts].find(o => o.dataset.code  === code);           if (opt) this._select(opt); }
}


// ============================================================
// UTILIDADES UI
// ============================================================

function showToast(msg, ms = 2400) {
  const t = document.getElementById('toast');
  t.textContent = msg;
  t.classList.add('show');
  clearTimeout(t._tid);
  t._tid = setTimeout(() => t.classList.remove('show'), ms);
}

// Abre un panel y cierra los demás; si ya estaba abierto, lo cierra.
function togglePanel(panelId, btnId) {
  const panel   = document.getElementById(panelId);
  const btn     = document.getElementById(btnId);
  const wasOpen = panel.classList.contains('visible');

  document.querySelectorAll('.panel').forEach(p => p.classList.remove('visible'));
  document.querySelectorAll('.btm-btn').forEach(b => b.classList.remove('active-btn'));
  document.querySelectorAll('.cs-wrap.open').forEach(w => w._cs?.close());

  if (!wasOpen) {
    panel.classList.add('visible');
    btn.classList.add('active-btn');
  }
}

// Sincroniza los inputs de ancho/alto al elegir una resolución predefinida
function syncResolution(value) {
  const m = value.match(/(\d+)\s*[xX×]\s*(\d+)/);
  if (m) {
    document.getElementById('width-input').value  = m[1];
    document.getElementById('height-input').value = m[2];
  }
}


// ============================================================
// FONDO Y LOGO PERSONALIZADOS
// — data URL → Blob → blob:// para evitar restricciones de Tauri
// ============================================================

const DEFAULT_BG = 'LauncherAssets/img/bg.png';

let _currentBgBlobUrl   = null;
let _currentLogoBlobUrl = null;

function dataUrlToObjectUrl(dataUrl) {
  const [header, b64] = dataUrl.split(',');
  const mime = header.match(/data:([^;]+)/)[1];
  const binary = atob(b64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return URL.createObjectURL(new Blob([bytes], { type: mime }));
}

async function applyCustomBg(dataUrlOrRelPath) {
  if (_currentBgBlobUrl) { URL.revokeObjectURL(_currentBgBlobUrl); _currentBgBlobUrl = null; }

  if (!dataUrlOrRelPath?.startsWith('data:')) {
    bg.style.backgroundImage = `url('${dataUrlOrRelPath || DEFAULT_BG}')`;
    return;
  }
  _currentBgBlobUrl = dataUrlToObjectUrl(dataUrlOrRelPath);
  bg.style.backgroundImage = `url('${_currentBgBlobUrl}')`;
}

async function applyCustomLogo(dataUrl) {
  if (_currentLogoBlobUrl) { URL.revokeObjectURL(_currentLogoBlobUrl); _currentLogoBlobUrl = null; }

  if (!dataUrl?.startsWith('data:')) {
    logoEl.style.backgroundImage = ''; // restaura el logo CSS por defecto
    return;
  }
  _currentLogoBlobUrl = dataUrlToObjectUrl(dataUrl);
  logoEl.style.backgroundImage = `url('${_currentLogoBlobUrl}')`;
}


// ============================================================
// INIT
// ============================================================

window.addEventListener('DOMContentLoaded', async () => {

  applyGlassFilters();

  // — Fondo personalizado —
  try {
    const savedBg = await invoke('get_custom_background');
    applyCustomBg(savedBg || DEFAULT_BG);
  } catch (_) { applyCustomBg(DEFAULT_BG); }

  document.getElementById('btn-change-bg').addEventListener('click', async () => {
    try {
      const newPath = await invoke('pick_and_set_background');
      if (!newPath) return;
      applyCustomBg(newPath);
      showToast('✓  Background updated');
    } catch (e) { console.error(e); showToast('✗  Could not change background'); }
  });

  // — Logo personalizado —
  try {
    const hideLogo = await invoke('get_hide_logo');
    const hideChk  = document.getElementById('toggle-hide-logo');
    if (hideChk) hideChk.checked = hideLogo;
    logoEl.style.display = hideLogo ? 'none' : '';
    if (!hideLogo) {
      const savedLogo = await invoke('get_custom_logo');
      if (savedLogo) applyCustomLogo(savedLogo);
    }
  } catch (_) {}

  document.getElementById('toggle-hide-logo')?.addEventListener('change', async (e) => {
    const hide = e.target.checked;
    logoEl.style.display = hide ? 'none' : '';
    try { await invoke('set_hide_logo', { hide }); } catch (err) { console.error(err); }
  });

  document.getElementById('btn-change-logo').addEventListener('click', async () => {
    try {
      const newLogo = await invoke('pick_and_set_logo');
      if (!newLogo) return;
      applyCustomLogo(newLogo);
      // Si el logo estaba oculto, mostrarlo al asignar uno nuevo
      const hideChk = document.getElementById('toggle-hide-logo');
      if (hideChk?.checked) {
        hideChk.checked = false;
        logoEl.style.display = '';
        await invoke('set_hide_logo', { hide: false });
      }
      showToast('✓  Logo updated');
    } catch (e) { console.error(e); showToast('✗  Could not change logo'); }
  });

  // — Referencias DOM —
  const widthInput      = document.getElementById('width-input');
  const heightInput     = document.getElementById('height-input');
  const fullscreenCheck = document.getElementById('fullscreen-check');
  const displayInput    = document.getElementById('display-input');
  const toggleConsole   = document.getElementById('toggle-console');
  const toggleRimRemover= document.getElementById('toggle-rimremover');
  const toggleAutosave  = document.getElementById('toggle-autosave');
  const toggleStiletto  = document.getElementById('toggle-stiletto');
  const themeIcon       = document.getElementById('theme-icon');

  // — Custom selects —
  const csSize    = new CustomSelect('size-select');
  const csQuality = new CustomSelect('quality-select');
  const csLang    = new CustomSelect('language-select');

  document.getElementById('size-select').addEventListener('cs:change', () =>
    syncResolution(csSize.value));

  // Cerrar dropdowns / paneles al hacer click fuera
  launcher.addEventListener('click', e => {
    if (!e.target.closest('.cs-wrap'))
      document.querySelectorAll('.cs-wrap.open').forEach(w => w._cs?.close());
    if (!e.target.closest('.panel') && !e.target.closest('#btn-settings, #btn-folders')) {
      document.querySelectorAll('.panel').forEach(p => p.classList.remove('visible'));
      document.querySelectorAll('.btm-btn').forEach(b => b.classList.remove('active-btn'));
    }
  });

  // — Tema dark / light —
  function applyTheme(theme) {
    launcher.dataset.theme = theme;
    themeIcon.src = theme === 'dark'
      ? 'LauncherAssets/img/lighttheme.svg'
      : 'LauncherAssets/img/darktheme.svg';
    localStorage.setItem('ks-theme', theme);
  }
  applyTheme(localStorage.getItem('ks-theme') || 'dark');

  document.getElementById('btn-theme').addEventListener('click', () => {
    const next = launcher.dataset.theme === 'dark' ? 'light' : 'dark';
    launcher.classList.add('theme-blur');
    setTimeout(() => applyTheme(next), 130); // cambia en el pico del blur
    launcher.addEventListener('animationend', () => launcher.classList.remove('theme-blur'), { once: true });
  });

  // — Controles de ventana —
  document.getElementById('btn-minimize').addEventListener('click', async () => {
    try { await appWindow?.minimize(); } catch (e) { console.error('minimize:', e); }
  });
  document.getElementById('btn-close').addEventListener('click', async () => {
    try { await appWindow?.close(); } catch (e) { console.error('close:', e); }
  });

  // — Arrastrar ventana —
  launcher.addEventListener('mousedown', async (e) => {
    if (e.button !== 0) return;
    if (e.target.closest('button, input, label, select, textarea, .cs-wrap, .cs-inner, .panel, .controls')) return;
    try { await appWindow?.startDragging(); } catch (err) { console.error('startDragging:', err); }
  });

  // — Paneles —
  document.getElementById('btn-settings').addEventListener('click', () => togglePanel('settings-panel', 'btn-settings'));
  document.getElementById('btn-folders').addEventListener('click',  () => togglePanel('folders-panel',  'btn-folders'));

  // — Lanzadores —
  document.getElementById('btn-game').addEventListener('click', async () => {
    try { await invoke('launch_game'); }
    catch (e) { console.error(e); showToast('Error launching the game'); }
  });
  document.getElementById('btn-studio').addEventListener('click', async () => {
    try { await invoke('launch_studio'); }
    catch (e) { console.error(e); showToast('Error launching Chara Studio'); }
  });

  // — Carpetas —
  document.querySelectorAll('.folder-btn').forEach(btn => {
    btn.addEventListener('click', async () => {
      try { await invoke('open_folder', { relativePath: btn.dataset.path }); }
      catch (e) { console.error(e); showToast('Could not open folder'); }
    });
  });

  // — Cargar configuración desde setup.xml —
  try {
    const cfg = await invoke('get_setup_xml');
    csSize.setValue(cfg.Size);
    widthInput.value        = cfg.Width;
    heightInput.value       = cfg.Height;
    csQuality.setValue(String(cfg.Quality));
    fullscreenCheck.checked = cfg.FullScreen;
    displayInput.value      = cfg.Display;
    csLang.setValue(String(cfg.Language));
  } catch (e) { console.error('get_setup_xml:', e); }

  try {
    const code = await invoke('get_current_language_from_ini');
    if (code) csLang.setByCode(code);
  } catch (e) { console.error('get_current_language_from_ini:', e); }

  try {
    await invoke('initialize_fonts', { languageCode: csLang.code || 'en' });
  } catch (e) { console.error('initialize_fonts:', e); }

  // — Guardar configuración —
  document.getElementById('btn-save').addEventListener('click', async () => {
    const cfg = {
      Size:       csSize.value,
      Width:      parseInt(widthInput.value)   || 1280,
      Height:     parseInt(heightInput.value)  || 720,
      Quality:    parseInt(csQuality.value)    || 0,
      FullScreen: fullscreenCheck.checked,
      Display:    parseInt(displayInput.value) || 1,
      Language:   parseInt(csLang.value)       || 0,
    };
    try {
      await invoke('save_setup_xml', { config: cfg, languageCode: csLang.code || 'en' });
      await invoke('set_fallback_font', { languageCode: csLang.code || 'en' });
      showToast('✓  Configuration saved');
    } catch (e) {
      console.error('save_setup_xml:', e);
      showToast('✗  Error saving configuration');
    }
  });

  // — Plugins y consola de BepInEx —
  async function loadPluginStates() {
    try {
      const s = await invoke('get_plugin_states');
      toggleRimRemover.checked = !!s.RimRemover;
      toggleAutosave.checked   = !!s.AutoSave;
      toggleStiletto.checked   = !!s.Stiletto;
    } catch (e) { console.error('get_plugin_states:', e); }
    try {
      toggleConsole.checked = await invoke('get_console_enabled');
    } catch (e) { console.error('get_console_enabled:', e); }
  }
  loadPluginStates();

  [
    [toggleConsole,     null,          'set_console_enabled'],
    [toggleRimRemover,  'RimRemover',  'toggle_plugin'],
    [toggleAutosave,    'AutoSave',    'toggle_plugin'],
    [toggleStiletto,    'Stiletto',    'toggle_plugin'],
  ].forEach(([el, pluginId, cmd]) => {
    el.addEventListener('change', async () => {
      try {
        await invoke(cmd, pluginId ? { pluginId, enable: el.checked } : { enable: el.checked });
      } catch (e) {
        console.error(e);
        el.checked = !el.checked; // revertir si falla
      }
    });
  });

}); // end DOMContentLoaded

// fin de main.js
// deadshark