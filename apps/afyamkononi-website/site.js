/* AfyaMkononi — shared site behaviour: glass navbar and app-screen carousels. */
(() => {
  'use strict';

  // Navbar: stronger glass once the page scrolls; mobile menu toggle.
  const header = document.querySelector('[data-am-header]');
  if (header) {
    const onScroll = () => header.classList.toggle('is-scrolled', window.scrollY > 8);
    onScroll();
    window.addEventListener('scroll', onScroll, { passive: true });
    const button = header.querySelector('.am-menu');
    const panel = header.querySelector('.am-mobile-nav');
    const setOpen = (open) => {
      button.setAttribute('aria-expanded', String(open));
      panel.classList.toggle('is-open', open);
      button.querySelector('.sr-only').textContent = open ? 'Close menu' : 'Open menu';
    };
    button?.addEventListener('click', () => setOpen(button.getAttribute('aria-expanded') !== 'true'));
    panel?.querySelectorAll('a').forEach((a) => a.addEventListener('click', () => setOpen(false)));
    document.addEventListener('keydown', (e) => { if (e.key === 'Escape') setOpen(false); });
  }

  // Account deletion form: see account-deletion.js (loaded on that page only).

  // Carousels: a phone of app screens with a synced description card.
  document.querySelectorAll('[data-am-carousel]').forEach((root) => {
    const track = root.querySelector('.am-track');
    const slides = Array.from(root.querySelectorAll('.am-slide'));
    const phone = root.querySelector('.am-phone');
    const kicker = root.querySelector('.am-card__kicker');
    const count = root.querySelector('.am-card__count b');
    const title = root.querySelector('.am-card h3');
    const desc = root.querySelector('.am-card__desc');
    const tags = root.querySelector('.am-tags');
    const dots = Array.from(root.querySelectorAll('.am-dot'));
    let index = 0;

    const render = (i, first = false) => {
      index = (i + slides.length) % slides.length;
      const s = slides[index].dataset;
      track.style.transform = `translateX(-${index * 100}%)`;
      slides.forEach((el, k) => el.setAttribute('aria-hidden', String(k !== index)));
      dots.forEach((d, k) => d.setAttribute('aria-selected', String(k === index)));
      kicker.textContent = s.kicker;
      count.textContent = String(index + 1).padStart(2, '0');
      const swap = () => {
        title.textContent = s.title;
        desc.textContent = s.desc;
        tags.replaceChildren(...s.tags.split('|').map((t) => {
          const li = document.createElement('li');
          li.textContent = t.trim();
          return li;
        }));
        title.style.opacity = desc.style.opacity = '1';
      };
      if (first) swap();
      else { title.style.opacity = desc.style.opacity = '0'; setTimeout(swap, 160); }
    };

    root.querySelector('.am-arrow.prev')?.addEventListener('click', () => render(index - 1));
    root.querySelector('.am-arrow.next')?.addEventListener('click', () => render(index + 1));
    dots.forEach((d, k) => d.addEventListener('click', () => render(k)));
    phone?.addEventListener('keydown', (e) => {
      const map = { ArrowLeft: index - 1, ArrowRight: index + 1, Home: 0, End: slides.length - 1 };
      if (e.key in map) { e.preventDefault(); render(map[e.key]); }
    });
    let x0 = 0, y0 = 0;
    phone?.addEventListener('touchstart', (e) => { x0 = e.changedTouches[0].clientX; y0 = e.changedTouches[0].clientY; }, { passive: true });
    phone?.addEventListener('touchend', (e) => {
      const dx = e.changedTouches[0].clientX - x0, dy = e.changedTouches[0].clientY - y0;
      if (Math.abs(dx) > 40 && Math.abs(dx) > Math.abs(dy)) render(index + (dx < 0 ? 1 : -1));
    }, { passive: true });
    render(0, true);
  });

  // Legal pages: buttons (no inline handlers, so the Content Security Policy
  // can forbid inline script), back-to-top visibility and the table-of-contents
  // highlight.
  document.addEventListener('click', (e) => {
    const btn = e.target.closest('[data-am-action]');
    if (!btn) return;
    const action = btn.dataset.amAction;
    if (action === 'print') window.print();
    else if (action === 'top') window.scrollTo({ top: 0, behavior: 'smooth' });
    else if (action === 'toggle-toc') {
      const drawer = document.getElementById('mobile-toc-drawer');
      if (!drawer) return;
      const open = drawer.style.display !== 'none';
      drawer.style.display = open ? 'none' : 'block';
      btn.setAttribute('aria-expanded', String(!open));
      const chevron = btn.querySelector('.lucide-chevron-down');
      if (chevron) chevron.style.transform = open ? 'rotate(0deg)' : 'rotate(180deg)';
    }
  });
  const backToTop = document.getElementById('back-to-top');
  const tocLinks = Array.from(document.querySelectorAll('aside nav[aria-label="Table of contents"] a'));
  const tocSections = tocLinks
    .map((link) => {
      const href = link.getAttribute('href') || '';
      const el = href.startsWith('#') ? document.getElementById(href.slice(1)) : null;
      return el && { id: el.id, el };
    })
    .filter(Boolean);
  if (backToTop || tocSections.length) {
    window.addEventListener('scroll', () => {
      backToTop?.classList.toggle('visible', window.scrollY > 400);
      let currentId = '';
      tocSections.forEach((sec) => { if (sec.el.getBoundingClientRect().top <= 140) currentId = sec.id; });
      if (currentId) {
        tocLinks.forEach((link) => link.classList.toggle('active-toc-link', link.getAttribute('href') === `#${currentId}`));
      }
    }, { passive: true });
  }

  // Scroll-spy: on the home page, underline the header link of the section
  // being read (Patients, Doctors, Peer navigators).
  const spyLinks = Array.from(document.querySelectorAll('.am-nav a[href^="/#"], .am-mobile-nav a[href^="/#"]'));
  const onHome = location.pathname === '/' || /\/index(\.html)?$/.test(location.pathname);
  const spySections = onHome
    ? [...new Set(spyLinks.map((a) => a.hash))].map((h) => document.querySelector(h)).filter(Boolean)
    : [];
  if (spySections.length) {
    let current = null;
    const setActive = (id) => {
      if (id === current) return;
      current = id;
      spyLinks.forEach((a) => {
        const on = a.hash === `#${id}`;
        a.classList.toggle('is-active', on);
        if (on) a.setAttribute('aria-current', 'location'); else a.removeAttribute('aria-current');
      });
    };
    let queued = false;
    const update = () => {
      queued = false;
      // The section crossing a line a third of the way down the screen.
      const line = window.innerHeight * 0.33;
      const hit = spySections.find((sec) => {
        const r = sec.getBoundingClientRect();
        return r.top <= line && r.bottom > line;
      });
      setActive(hit ? hit.id : null);
    };
    const queue = () => { if (!queued) { queued = true; requestAnimationFrame(update); } };
    window.addEventListener('scroll', queue, { passive: true });
    window.addEventListener('resize', queue);
    update();
  }

  // Motion: page-to-page fades, header entrance, scroll reveals.
  // Skipped entirely when the visitor prefers reduced motion.
  const reduceMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;

  // A page reached through a cross-fade keeps its header still.
  window.addEventListener('pagereveal', (e) => {
    if (e.viewTransition) document.documentElement.classList.add('am-vt');
  });

  if (!reduceMotion) {
    const bar = document.createElement('div');
    bar.className = 'am-progress';
    bar.setAttribute('aria-hidden', 'true');
    document.body.prepend(bar);
  }

  if (!reduceMotion && 'IntersectionObserver' in window) {
    const BLOCKS = 'h1,h2,h3,p,ul,ol,table,figure,img,picture,article,form,details,blockquote,'
      + '.am-phone,.am-card,.am-actions,.am-controls,.v2-actions,.page-actions,.am-footer__col';
    const ZOOM = 'img,picture,figure,.am-phone';
    const picked = [];
    document.querySelectorAll('main, .am-footer').forEach((scope) => {
      scope.querySelectorAll(BLOCKS).forEach((el) => {
        // Parents come first, so anything inside a picked block moves with it.
        if (el.parentElement.closest('.am-reveal') || el.closest('.am-track')) return;
        if (!el.getClientRects().length) return;
        // Long blocks (a whole policy, a big table) would fade in as one
        // slab; let their headings and paragraphs move instead.
        if (el.getBoundingClientRect().height > window.innerHeight * 1.1) return;
        el.classList.add('am-reveal');
        if (el.matches(ZOOM)) el.classList.add('am-reveal--zoom');
        picked.push(el);
      });
    });

    // Once shown, hand the element back to its own styles.
    const settle = (el) => {
      el.classList.remove('am-reveal', 'am-reveal--zoom', 'is-in');
      el.style.removeProperty('--am-d');
    };
    const io = new IntersectionObserver((entries) => {
      entries.filter((e) => e.isIntersecting)
        .sort((a, b) => a.boundingClientRect.top - b.boundingClientRect.top || a.boundingClientRect.left - b.boundingClientRect.left)
        .forEach((entry, i) => {
          const el = entry.target;
          io.unobserve(el);
          const delay = Math.min(i, 6) * 80;
          el.style.setProperty('--am-d', `${delay}ms`);
          el.classList.add('is-in');
          setTimeout(() => settle(el), delay + 900);
        });
    }, { threshold: 0, rootMargin: '0px 0px -8% 0px' });
    picked.forEach((el) => io.observe(el));

  }

})();
