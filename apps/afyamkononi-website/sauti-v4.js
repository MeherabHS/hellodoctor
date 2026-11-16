/**
 * AfyaMkononi - Sauti V4 Interactive Scripts
 * Controls mobile navigation, scroll reveal, and accessible product carousels.
 */

(() => {
  'use strict';

  // --- Mobile Navigation ---
  const menuButton = document.querySelector('.v2-menu-toggle');
  const mobileNav = document.querySelector('.v2-mobile-nav');

  const setMenu = (open) => {
    menuButton?.classList.toggle('is-open', open);
    mobileNav?.classList.toggle('is-open', open);
    menuButton?.setAttribute('aria-expanded', String(open));
    const label = menuButton?.querySelector('.sr-only');
    if (label) label.textContent = open ? 'Close menu' : 'Open menu';
  };

  menuButton?.addEventListener('click', () => {
    setMenu(menuButton.getAttribute('aria-expanded') !== 'true');
  });

  mobileNav?.querySelectorAll('a').forEach((link) => {
    link.addEventListener('click', () => setMenu(false));
  });

  // --- Scroll Reveal Animations ---
  const revealItems = document.querySelectorAll('[data-reveal]');
  revealItems.forEach((item) => {
    item.style.setProperty('--reveal-delay', `${item.dataset.delay || 0}ms`);
  });

  if (window.matchMedia('(prefers-reduced-motion: reduce)').matches || !('IntersectionObserver' in window)) {
    revealItems.forEach((item) => item.classList.add('is-visible'));
  } else {
    const observer = new IntersectionObserver((entries, currentObserver) => {
      entries.forEach((entry) => {
        if (entry.isIntersecting) {
          entry.target.classList.add('is-visible');
          currentObserver.unobserve(entry.target);
        }
      });
    }, { threshold: 0.16, rootMargin: '0px 0px -40px' });
    revealItems.forEach((item) => observer.observe(item));
  }

  // --- Interactive Product Carousels ---
  class ProductCarousel {
    constructor(container) {
      this.container = container;
      this.track = container.querySelector('.v4-carousel-track');
      this.slides = container.querySelectorAll('.v4-carousel-slide');
      this.screen = container.querySelector('.v4-carousel-screen');
      this.phone = container.querySelector('.v4-carousel-phone');
      
      this.counterEl = container.querySelector('.v4-active-num');
      this.kickerEl = container.querySelector('.v4-editorial-kicker');
      this.titleEl = container.querySelector('.v4-editorial-title');
      this.descEl = container.querySelector('.v4-editorial-desc');
      this.tagsContainer = container.querySelector('.v4-editorial-tags');
      
      this.prevBtn = container.querySelector('.v4-arrow-btn.prev');
      this.nextBtn = container.querySelector('.v4-arrow-btn.next');
      this.pills = container.querySelectorAll('.v4-nav-pill');

      this.currentIndex = 0;
      this.totalSlides = this.slides.length;
      if (this.totalSlides === 0) return;

      // Extract slide data from data attributes or DOM
      this.slideData = Array.from(this.slides).map((slide, idx) => ({
        indexStr: String(idx + 1).padStart(2, '0'),
        kicker: slide.dataset.kicker || `0${idx + 1} / FEATURE`,
        title: slide.dataset.title || '',
        desc: slide.dataset.desc || '',
        tags: (slide.dataset.tags || '').split(',').map((t) => t.trim()).filter(Boolean)
      }));

      this.initEvents();
      this.updateView(0, true);
    }

    goTo(index) {
      if (index < 0) {
        this.currentIndex = this.totalSlides - 1;
      } else if (index >= this.totalSlides) {
        this.currentIndex = 0;
      } else {
        this.currentIndex = index;
      }
      this.updateView(this.currentIndex);
    }

    next() {
      this.goTo(this.currentIndex + 1);
    }

    prev() {
      this.goTo(this.currentIndex - 1);
    }

    updateView(index, isInit = false) {
      // 1. Move track
      if (this.track) {
        this.track.style.transform = `translateX(-${index * 100}%)`;
      }

      // 2. Update slides ARIA
      this.slides.forEach((slide, i) => {
        const isActive = i === index;
        slide.setAttribute('aria-hidden', String(!isActive));
      });

      // 3. Update editorial copy with subtle cross-fade
      const data = this.slideData[index];
      if (data) {
        if (this.counterEl) this.counterEl.textContent = data.indexStr;
        if (this.kickerEl) this.kickerEl.textContent = data.kicker;
        
        if (this.titleEl) {
          if (!isInit) {
            this.titleEl.style.opacity = '0';
            setTimeout(() => {
              this.titleEl.textContent = data.title;
              this.titleEl.style.opacity = '1';
            }, 120);
          } else {
            this.titleEl.textContent = data.title;
          }
        }

        if (this.descEl) {
          if (!isInit) {
            this.descEl.style.opacity = '0';
            setTimeout(() => {
              this.descEl.textContent = data.desc;
              this.descEl.style.opacity = '1';
            }, 120);
          } else {
            this.descEl.textContent = data.desc;
          }
        }

        if (this.tagsContainer) {
          this.tagsContainer.replaceChildren(...data.tags.map((tag) => {
            const pill = document.createElement('span');
            pill.className = 'v4-tag-pill';
            pill.textContent = tag;
            return pill;
          }));
        }
      }

      // 4. Update Indicator pills
      this.pills.forEach((pill, i) => {
        const isActive = i === index;
        pill.classList.toggle('active', isActive);
        pill.setAttribute('aria-selected', String(isActive));
      });
    }

    initEvents() {
      // Navigation buttons
      this.prevBtn?.addEventListener('click', () => this.prev());
      this.nextBtn?.addEventListener('click', () => this.next());

      // Pill indicators
      this.pills.forEach((pill) => {
        pill.addEventListener('click', () => {
          const target = parseInt(pill.dataset.slide, 10);
          if (!isNaN(target)) this.goTo(target);
        });
      });

      // Keyboard accessibility on phone frame
      if (this.phone) {
        this.phone.addEventListener('keydown', (e) => {
          if (e.key === 'ArrowLeft') {
            e.preventDefault();
            this.prev();
          } else if (e.key === 'ArrowRight') {
            e.preventDefault();
            this.next();
          } else if (e.key === 'Home') {
            e.preventDefault();
            this.goTo(0);
          } else if (e.key === 'End') {
            e.preventDefault();
            this.goTo(this.totalSlides - 1);
          }
        });
      }

      // Touch / Swipe handling
      if (this.screen) {
        let touchStartX = 0;
        let touchStartY = 0;
        let touchEndX = 0;
        let touchEndY = 0;

        this.screen.addEventListener('touchstart', (e) => {
          touchStartX = e.changedTouches[0].screenX;
          touchStartY = e.changedTouches[0].screenY;
        }, { passive: true });

        this.screen.addEventListener('touchend', (e) => {
          touchEndX = e.changedTouches[0].screenX;
          touchEndY = e.changedTouches[0].screenY;
          this.handleGesture(touchStartX, touchStartY, touchEndX, touchEndY);
        }, { passive: true });
      }
    }

    handleGesture(startX, startY, endX, endY) {
      const diffX = endX - startX;
      const diffY = endY - startY;

      // Check if horizontal swipe exceeds vertical movement and threshold (40px)
      if (Math.abs(diffX) > Math.abs(diffY) && Math.abs(diffX) > 40) {
        if (diffX < 0) {
          this.next(); // swipe left -> next slide
        } else {
          this.prev(); // swipe right -> prev slide
        }
      }
    }
  }

  // Initialize all carousels on page load
  document.addEventListener('DOMContentLoaded', () => {
    document.querySelectorAll('[data-product-carousel]').forEach((el) => {
      new ProductCarousel(el);
    });
  });

  // If DOM is already loaded
  if (document.readyState === 'interactive' || document.readyState === 'complete') {
    document.querySelectorAll('[data-product-carousel]').forEach((el) => {
      new ProductCarousel(el);
    });
  }
})();
