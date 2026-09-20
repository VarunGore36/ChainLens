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

// ====== LOAD RESULTS JSON ======
async function loadResults() {
    try {
        const res = await fetch('results.json?' + Date.now());
        if (!res.ok) return;
        const data = await res.json();
        applyResults(data);
    } catch (e) {
        console.log('No results.json found — using static values');
    }
}

function applyResults(d) {
    // Update stat counters
    const stats = {
        'pipeline_bps': d.pipeline_throughput?.blocks_per_sec,
        'decode_bps': d.decode_throughput?.blocks_per_sec,
        'tests_total': d.tests?.total,
        'decode_ns': d.decode_throughput?.eip1559_erc20_ns
    };

    document.querySelectorAll('[data-stat]').forEach(el => {
        const key = el.dataset.stat;
        if (stats[key] !== undefined) {
            el.dataset.target = stats[key];
            el.textContent = fmt(stats[key]);
        }
    });

    // Update test modules
    if (d.tests?.modules) {
        const mods = d.tests.modules;
        const testMap = {
            'config': mods.config,
            'decode/block': mods.decode_block,
            'decode/erc20': mods.decode_erc20,
            'rpc': mods.rpc,
            'store': mods.store,
            'domain': mods.domain,
            'reorg': mods.reorg,
            'integration': mods.integration,
            'crash recovery': mods.crash_recovery
        };

        document.querySelectorAll('.test-mod').forEach(el => {
            const name = el.querySelector('.name')?.textContent?.trim();
            if (name && testMap[name] !== undefined) {
                el.querySelector('.cnt').textContent = testMap[name];
            }
        });
    }

    // Update test counts
    if (d.tests) {
        const totalEl = document.querySelector('[data-test-total]');
        const failEl = document.querySelector('[data-test-fail]');
        const warnEl = document.querySelector('[data-test-warn]');
        if (totalEl) totalEl.textContent = d.tests.total;
        if (failEl) failEl.textContent = d.tests.failed;
        if (warnEl) warnEl.textContent = d.tests.clippy_warnings;
    }

    // Update bars from results
    if (d.pipeline_throughput) {
        const pt = d.pipeline_throughput;
        const maxBps = pt.workers_1;
        const barData = [
            { label: '1 worker', value: pt.workers_1, pct: 100 },
            { label: '2 workers', value: pt.workers_2, pct: (pt.workers_2 / maxBps * 100) },
            { label: '4 workers', value: pt.workers_4, pct: (pt.workers_4 / maxBps * 100) },
            { label: '8 workers', value: pt.workers_8, pct: (pt.workers_8 / maxBps * 100) },
            { label: '16 workers', value: pt.workers_16, pct: (pt.workers_16 / maxBps * 100) }
        ];

        const pipelineCard = document.querySelectorAll('.bench-card')[0];
        if (pipelineCard) {
            const bars = pipelineCard.querySelectorAll('.bar-fill');
            bars.forEach((bar, i) => {
                if (barData[i]) {
                    bar.dataset.width = Math.round(barData[i].pct);
                    bar.textContent = barData[i].value + ' b/s';
                    bar.className = 'bar-fill ' + (barData[i].pct > 50 ? 'blue' : 'amber');
                }
            });
        }
    }

    if (d.decode_throughput) {
        const dt = d.decode_throughput;
        const maxNs = dt.multi_tx_logs_ns;
        const decodeData = [
            { value: dt.empty_block_ns },
            { value: dt.legacy_tx_ns },
            { value: dt.contract_create_ns },
            { value: dt.erc721_ns },
            { value: dt.eip1559_erc20_ns },
            { value: dt.multi_tx_logs_ns }
        ];

        const decodeCard = document.querySelectorAll('.bench-card')[1];
        if (decodeCard) {
            const bars = decodeCard.querySelectorAll('.bar-fill');
            bars.forEach((bar, i) => {
                if (decodeData[i]) {
                    bar.dataset.width = Math.round(decodeData[i].value / maxNs * 100);
                    bar.textContent = decodeData[i].value + 'ns';
                }
            });
        }
    }

    // Update commit badge
    if (d.commit) {
        const badge = document.querySelector('.hero-badge span');
        if (badge) {
            badge.textContent = `${d.commit} · ${d.generated_at?.split('T')[0] || 'live'}`;
        }
    }

    // Update big stat
    if (d.decode_throughput?.blocks_per_sec) {
        const bigNum = document.querySelector('.big-stat .num');
        if (bigNum) {
            bigNum.dataset.target = d.decode_throughput.blocks_per_sec;
            bigNum.textContent = fmt(d.decode_throughput.blocks_per_sec);
        }
    }
}

// Load on page ready
loadResults();

// ====== WEBSOCKET ======
let ws = null;
let wsReconnectTimer = null;

function connectWebSocket() {
    const wsUrl = window.location.protocol === 'https:'
        ? `wss://${window.location.host}/ws`
        : `ws://${window.location.hostname}:8080/ws`;

    try {
        ws = new WebSocket(wsUrl);

        ws.onopen = () => {
            console.log('WebSocket connected');
            addNotification('Connected to live feed', 'success');
        };

        ws.onmessage = (event) => {
            try {
                const data = JSON.parse(event.data);
                handleWsEvent(data);
            } catch (e) {
                console.error('Failed to parse WebSocket message:', e);
            }
        };

        ws.onclose = () => {
            console.log('WebSocket disconnected');
            wsReconnectTimer = setTimeout(connectWebSocket, 5000);
        };

        ws.onerror = (error) => {
            console.error('WebSocket error:', error);
        };
    } catch (e) {
        console.log('WebSocket not available');
    }
}

function handleWsEvent(data) {
    switch (data.type) {
        case 'connected':
            break;
        case 'block_committed':
            addNotification(`Block #${data.block_number} committed (${data.tx_count} txs)`, 'info');
            break;
        case 'anomaly_detected':
            addNotification(`Anomaly: ${data.description}`, data.severity === 'high' ? 'error' : 'warning');
            break;
        case 'mev_detected':
            addNotification(`MEV: ${data.description}`, 'warning');
            break;
        case 'reorg_detected':
            addNotification(`Reorg detected at block ${data.common_ancestor}`, 'error');
            break;
    }
}

function addNotification(message, type = 'info') {
    const container = document.getElementById('notifications');
    if (!container) return;

    const notification = document.createElement('div');
    notification.className = `notification ${type}`;
    notification.innerHTML = `
        <div class="notification-content">
            <span class="notification-icon">${type === 'error' ? '🔴' : type === 'warning' ? '🟡' : '🟢'}</span>
            <span class="notification-text">${message}</span>
        </div>
        <button class="notification-close" onclick="this.parentElement.remove()">×</button>
    `;

    container.appendChild(notification);

    setTimeout(() => {
        notification.classList.add('fade-out');
        setTimeout(() => notification.remove(), 300);
    }, 5000);
}

// ====== EXPORT FUNCTIONS ======
function exportData(format, endpoint) {
    const url = `${API_BASE}${endpoint}?format=${format}&limit=10000`;

    const a = document.createElement('a');
    a.href = url;
    a.download = `chainlens-export.${format}`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
}

function exportAddressTransactions(address, format = 'json') {
    exportData(format, `/api/v1/addresses/${address}/export`);
}

function exportAnomalies(format = 'json') {
    window.open(`${API_BASE}/api/v1/anomalies?format=${format}`, '_blank');
}
