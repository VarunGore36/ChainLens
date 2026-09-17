// ====== PARTICLE BACKGROUND ======
const canvas = document.getElementById('bg');
const ctx = canvas.getContext('2d');
let W, H, particles = [];

function resize() {
    W = canvas.width = innerWidth;
    H = canvas.height = innerHeight;
}
resize();
addEventListener('resize', resize);

class Particle {
    constructor() { this.reset(); }
    reset() {
        this.x = Math.random() * W;
        this.y = Math.random() * H;
        this.vx = (Math.random() - .5) * .3;
        this.vy = (Math.random() - .5) * .3;
        this.r = Math.random() * 1.5 + .5;
        this.a = Math.random() * .4 + .1;
    }
    update() {
        this.x += this.vx;
        this.y += this.vy;
        if (this.x < 0 || this.x > W) this.vx *= -1;
        if (this.y < 0 || this.y > H) this.vy *= -1;
    }
    draw() {
        ctx.beginPath();
        ctx.arc(this.x, this.y, this.r, 0, Math.PI * 2);
        ctx.fillStyle = `rgba(98,126,234,${this.a})`;
        ctx.fill();
    }
}

for (let i = 0; i < 80; i++) particles.push(new Particle());

function drawLines() {
    for (let i = 0; i < particles.length; i++) {
        for (let j = i + 1; j < particles.length; j++) {
            const dx = particles[i].x - particles[j].x;
            const dy = particles[i].y - particles[j].y;
            const d = Math.sqrt(dx * dx + dy * dy);
            if (d < 150) {
                ctx.beginPath();
                ctx.moveTo(particles[i].x, particles[i].y);
                ctx.lineTo(particles[j].x, particles[j].y);
                ctx.strokeStyle = `rgba(98,126,234,${.08 * (1 - d / 150)})`;
                ctx.stroke();
            }
        }
    }
}

function animate() {
    ctx.clearRect(0, 0, W, H);
    particles.forEach(p => { p.update(); p.draw(); });
    drawLines();
    requestAnimationFrame(animate);
}
animate();

// ====== NAV SCROLL ======
const nav = document.getElementById('nav');
addEventListener('scroll', () => {
    nav.classList.toggle('scrolled', scrollY > 50);
});

// ====== SCROLL REVEAL ======
const revealObs = new IntersectionObserver((entries) => {
    entries.forEach(entry => {
        if (entry.isIntersecting) {
            entry.target.classList.add('visible');
            revealObs.unobserve(entry.target);
        }
    });
}, { threshold: .15 });

document.querySelectorAll('.reveal').forEach(el => revealObs.observe(el));

// ====== COUNTER ANIMATION ======
function animateNum(el, target, dur = 2000) {
    let s = 0;
    const inc = target / (dur / 16);
    const timer = setInterval(() => {
        s += inc;
        if (s >= target) {
            el.textContent = fmt(target);
            clearInterval(timer);
        } else {
            el.textContent = fmt(Math.floor(s));
        }
    }, 16);
}

function fmt(n) {
    if (n >= 1e6) return (n / 1e6).toFixed(1) + 'M';
    if (n >= 1e3) return (n / 1e3).toFixed(0) + 'K';
    return n.toLocaleString();
}

const statsObs = new IntersectionObserver((entries) => {
    entries.forEach(entry => {
        if (entry.isIntersecting) {
            entry.target.querySelectorAll('[data-target]').forEach(c => {
                animateNum(c, +c.dataset.target);
            });
            statsObs.unobserve(entry.target);
        }
    });
}, { threshold: .5 });

document.querySelectorAll('.stat, .big-stat').forEach(el => statsObs.observe(el));

// ====== BAR ANIMATION ======
const barObs = new IntersectionObserver((entries) => {
    entries.forEach(entry => {
        if (entry.isIntersecting) {
            entry.target.querySelectorAll('.bar-fill').forEach(b => {
                b.style.width = b.dataset.width + '%';
            });
            barObs.unobserve(entry.target);
        }
    });
}, { threshold: .3 });

document.querySelectorAll('.bench-card').forEach(el => barObs.observe(el));

// ====== COPY BUTTON ======
document.getElementById('copyBtn').addEventListener('click', function() {
    const code = document.getElementById('code').textContent.trim();
    navigator.clipboard.writeText(code).then(() => {
        this.textContent = 'Copied!';
        setTimeout(() => this.textContent = 'Copy', 2000);
    });
});

// ====== SMOOTH SCROLL ======
document.querySelectorAll('a[href^="#"]').forEach(a => {
    a.addEventListener('click', e => {
        e.preventDefault();
        const target = document.querySelector(a.getAttribute('href'));
        if (target) target.scrollIntoView({ behavior: 'smooth' });
    });
});
