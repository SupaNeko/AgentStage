import { invoke } from '@tauri-apps/api/core';

function stringify(args: unknown[]): string {
    return args
        .map((a) => {
            if (a instanceof Error) return a.stack || a.message;
            if (typeof a === 'object') return JSON.stringify(a);
            return String(a);
        })
        .join(' ');
}

export function log(level: string, ...args: unknown[]) {
    const message = stringify(args);
    // Still output to browser console for devtools visibility
    const consoleFn = (console as unknown as Record<string, unknown>)[level.toLowerCase()] as typeof console.log;
    if (consoleFn) {
        consoleFn(`[${level}]`, ...args);
    } else {
        console.log(`[${level}]`, ...args);
    }
    // Send to backend log file
    invoke('log_frontend', { level, message }).catch(() => {});
}

export function debug(...args: unknown[]) {
    log('DEBUG', ...args);
}

export function info(...args: unknown[]) {
    log('INFO', ...args);
}

export function warn(...args: unknown[]) {
    log('WARN', ...args);
}

export function error(...args: unknown[]) {
    log('ERROR', ...args);
}

export const logger = { debug, info, warn, error };
