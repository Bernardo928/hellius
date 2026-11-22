import {
  useId,
  useRef,
  type ButtonHTMLAttributes,
  type HTMLAttributes,
  type InputHTMLAttributes,
  type KeyboardEvent,
  type Ref,
  type ReactNode,
} from 'react';
import './glass.css';
import { useSpecular } from './useSpecular';

const cx = (...parts: (string | false | undefined | null)[]) => parts.filter(Boolean).join(' ');

/* ---------- Botão ---------- */
export type GlassButtonVariant = 'primary' | 'secondary' | 'ghost' | 'danger';
export type GlassButtonSize = 'sm' | 'md' | 'lg';

interface GlassButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: GlassButtonVariant;
  size?: GlassButtonSize;
  icon?: ReactNode;
  /** Botão só com ícone: exige aria-label. */
  iconOnly?: boolean;
  /** Vidro flutuante com blur próprio. Padrão false: o contêiner pai já faz o blur. */
  floating?: boolean;
  pressed?: boolean;
}

export function GlassButton({
  variant = 'secondary',
  size = 'md',
  icon,
  iconOnly = false,
  floating = false,
  pressed,
  className,
  children,
  ...rest
}: GlassButtonProps) {
  const ref = useRef<HTMLButtonElement>(null);
  useSpecular(ref);
  return (
    <button
      ref={ref}
      type="button"
      className={cx(
        'glass-button',
        `glass-button--${size}`,
        variant !== 'secondary' && `glass-button--${variant}`,
        iconOnly && 'glass-button--icon',
        floating && 'glass-floating',
        className,
      )}
      aria-pressed={pressed === undefined ? undefined : pressed}
      {...rest}
    >
      {icon}
      {!iconOnly && children}
    </button>
  );
}

/* ---------- Painel ---------- */
interface GlassPanelProps extends HTMLAttributes<HTMLDivElement> {
  /** Blur de verdade. Use só em contêineres flutuantes (barras, painéis laterais). */
  floating?: boolean;
  as?: 'div' | 'section' | 'header' | 'footer' | 'aside';
}

export function GlassPanel({ floating = true, className, children, as = 'div', ...rest }: GlassPanelProps) {
  const ref = useRef<HTMLElement>(null);
  useSpecular(ref);
  const Tag = as as 'div';
  return (
    <Tag ref={ref as Ref<HTMLDivElement>} className={cx('glass', 'glass-panel', floating && 'glass-floating', className)} {...rest}>
      <div className="glass__content" style={{ position: 'relative', zIndex: 1, height: '100%' }}>
        {children}
      </div>
    </Tag>
  );
}

/* ---------- Campo ---------- */
export function GlassInput({ className, ...rest }: InputHTMLAttributes<HTMLInputElement>) {
  return <input className={cx('glass-input', className)} {...rest} />;
}

/* ---------- Abas ---------- */
export interface GlassTab<T extends string> {
  id: T;
  label: string;
}

interface GlassTabsProps<T extends string> {
  tabs: GlassTab<T>[];
  value: T;
  onChange: (id: T) => void;
  label: string;
  className?: string;
}

export function GlassTabs<T extends string>({ tabs, value, onChange, label, className }: GlassTabsProps<T>) {
  const onKey = (e: KeyboardEvent<HTMLDivElement>) => {
    const idx = tabs.findIndex((t) => t.id === value);
    if (e.key === 'ArrowRight' || e.key === 'ArrowLeft') {
      e.preventDefault();
      const dir = e.key === 'ArrowRight' ? 1 : -1;
      const next = tabs[(idx + dir + tabs.length) % tabs.length];
      if (next) onChange(next.id);
    }
  };
  return (
    <div role="tablist" aria-label={label} className={cx('glass-tabs', className)} onKeyDown={onKey}>
      {tabs.map((t) => (
        <button
          key={t.id}
          role="tab"
          type="button"
          className="glass-tab"
          aria-selected={t.id === value}
          tabIndex={t.id === value ? 0 : -1}
          onClick={() => onChange(t.id)}
        >
          {t.label}
        </button>
      ))}
    </div>
  );
}

/* ---------- Toggle ---------- */
interface GlassToggleProps {
  checked: boolean;
  onChange: (v: boolean) => void;
  label: string;
}

export function GlassToggle({ checked, onChange, label }: GlassToggleProps) {
  return (
    <button
      type="button"
      role="switch"
      className="glass-toggle"
      aria-checked={checked}
      aria-label={label}
      onClick={() => onChange(!checked)}
    />
  );
}

/* ---------- Chip ---------- */
interface GlassChipProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  meta?: string;
}

export function GlassChip({ meta, children, className, ...rest }: GlassChipProps) {
  return (
    <button type="button" className={cx('glass-chip', className)} {...rest}>
      <span>{children}</span>
      {meta && <span className="glass-chip__meta">{meta}</span>}
    </button>
  );
}

/* ---------- Toast ---------- */
export type ToastTone = 'info' | 'error' | 'success';

interface GlassToastProps {
  tone?: ToastTone;
  children: ReactNode;
  action?: ReactNode;
}

export function GlassToast({ tone = 'info', children, action }: GlassToastProps) {
  const ref = useRef<HTMLDivElement>(null);
  useSpecular(ref);
  return (
    <div
      ref={ref}
      role={tone === 'error' ? 'alert' : 'status'}
      className={cx('glass', 'glass-floating', 'glass-toast', tone !== 'info' && `glass-toast--${tone}`)}
    >
      <div style={{ position: 'relative', zIndex: 1, display: 'flex', gap: 10, alignItems: 'flex-start' }}>
        <div style={{ flex: 1 }}>{children}</div>
        {action}
      </div>
    </div>
  );
}

/* ---------- Tooltip ---------- */
export function GlassTooltip({ label, children }: { label: string; children: ReactNode }) {
  const id = useId();
  return (
    <span className="glass-tooltip-host">
      <span aria-describedby={id}>{children}</span>
      <span role="tooltip" id={id} className="glass-tooltip">
        {label}
      </span>
    </span>
  );
}
