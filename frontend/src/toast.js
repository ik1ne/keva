'use strict';

export function showToast(message) {
    // Skip if same message is already showing
    const existing = document.querySelector('.toast');
    if (existing && existing.textContent === message) return;

    const toast = document.createElement('div');
    toast.className = 'toast';
    toast.textContent = message;
    document.body.appendChild(toast);

    setTimeout(function () {
        toast.classList.add('fade-out');
        setTimeout(function () {
            toast.remove();
        }, 300);
    }, 2000);
}
