import {
  cloneElement,
  isValidElement,
  useId,
  useRef,
  type ButtonHTMLAttributes,
  type HTMLAttributes,
  type InputHTMLAttributes,
  type KeyboardEvent,
  type ReactElement,
  type ReactNode,
  type RefObject,
} from 'react';
import './glass.css';
import { useSpecular } from './useSpecular';
import { IconSearch } from '../icons';

const cx = (...parts: (string | false | undefined | null)[]) => parts.filter(Boolean).join(' ');

/* ---------- Botão ----------
   plain: padrão. Sem borda nem fundo em repouso; é o botão que vive dentro do vidro.
   primary: destaque com cor no fundo. Use no máximo um por tela.
   secondary: botão independente sobre o conteúdo (fora do vidro).
   danger: ação destrutiva. */
export type GlassButtonVariant = 'plain' | 'primary' | 'secondary' | 'danger';
export type GlassButtonSize = 'sm' | 'md' | 'lg';

interface GlassButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: GlassButtonVariant;
  size?: GlassButtonSize;
  icon?: ReactNode;
  /** Só ícone: exige aria-label. */
  iconOnly?: boolean;
  pressed?: boolean;
}

export function GlassButton({
  variant = 'plain',
  size = 'md',
  icon,
  iconOnly = false,
  pressed,
  className,
  children,
  onClick,
  ...rest
}: GlassButtonProps) {
  // aria-disabled mantém o botão focável (para ler a dica), mas ignora o clique.
  const ariaDisabled = rest['aria-disabled'] === true || rest['aria-disabled'] === 'true';
  return (
    <button
      type="button"
      className={cx(
        'glass-button',
        size !== 'md' && `glass-button--${size}`,
        variant !== 'plain' && `glass-button--${variant}`,
        iconOnly && 'glass-button--icon',
        className,
      )}
      aria-pressed={pressed === undefined ? undefined : pressed}
      onClick={ariaDisabled ? (e) => e.preventDefault() : onClick}
      {...rest}
    >
      {icon}
      {!iconOnly && children}
    </button>
  );
}

/* ---------- Painel de vidro (camada funcional) ---------- */
interface GlassPanelProps extends HTMLAttributes<HTMLElement> {
  as?: 'div' | 'section' | 'header' | 'footer' | 'aside' | 'nav';
}

export function GlassPanel({ className, children, as = 'div', ...rest }: GlassPanelProps) {
  const ref = useRef<HTMLElement>(null);
  useSpecular(ref);
  const Tag = as as 'div';
  return (
    <Tag ref={ref as RefObject<HTMLDivElement>} className={cx('glass', className)} {...rest}>
      {children}
    </Tag>
  );
}

/* ---------- Campo ---------- */
export function GlassInput({ className, ...rest }: InputHTMLAttributes<HTMLInputElement>) {
  return <input className={cx('glass-input', className)} {...rest} />;
}

/** Campo de busca com ícone de lupa. */
export function GlassSearchField({ className, ...rest }: InputHTMLAttributes<HTMLInputElement>) {
  return (
    <span className={cx('glass-field', className)}>
      <IconSearch size={14} />
      <input type="search" className="glass-input" {...rest} />
    </span>
  );
}

/* ---------- Controle segmentado ---------- */
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
  id?: string;
}

export function GlassToggle({ checked, onChange, label, id }: GlassToggleProps) {
  return (
    <button
      id={id}
      type="button"
      role="switch"
      className="glass-toggle"
      aria-checked={checked}
      aria-label={label}
      onClick={() => onChange(!checked)}
    />
  );
}

/* ---------- Chip interativo (só fora do vidro) ---------- */
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
  return (
    <GlassPanel role={tone === 'error' ? 'alert' : 'status'} className={cx('glass-toast', `glass-toast--${tone}`)}>
      <span className="glass-toast__dot" aria-hidden="true" />
      <div style={{ flex: 1 }}>{children}</div>
      {action}
    </GlassPanel>
  );
}

/* ---------- Tooltip ----------
   A descrição vai no próprio controle (aria-describedby), não no invólucro. */
export function GlassTooltip({ label, children }: { label: string; children: ReactElement }) {
  const id = useId();
  const child = isValidElement<{ 'aria-describedby'?: string }>(children)
    ? cloneElement(children, { 'aria-describedby': id })
    : children;
  return (
    <span className="glass-tooltip-host">
      {child}
      <span role="tooltip" id={id} className="glass-tooltip">
        {label}
      </span>
    </span>
  );
}
