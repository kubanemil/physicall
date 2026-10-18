export interface ParamSpec {
  key: string;
  label: string;
  unit: string;
  min: number;
  max: number;
  default: number;
  step: number;
  log: boolean;
  rebuild: boolean;
}

export interface SceneSpec {
  id: string;
  label: string;
  params: ParamSpec[];
}

/// Sliders for one scene. Log parameters slide over their exponent; rebuild
/// parameters apply when the slider is released, live ones while it moves.
export function buildParams(
  container: HTMLElement,
  scene: SceneSpec,
  onChange: (key: string, value: number) => void,
): void {
  container.replaceChildren(...scene.params.map((p) => control(p, onChange)));
}

function control(p: ParamSpec, onChange: (key: string, value: number) => void): HTMLElement {
  const label = document.createElement('label');
  label.className = 'param';
  const row = document.createElement('span');
  row.className = 'row';
  const name = document.createElement('span');
  name.textContent = p.unit ? `${p.label} (${p.unit})` : p.label;
  const output = document.createElement('output');
  row.append(name, output);

  const input = document.createElement('input');
  input.type = 'range';
  if (p.log) {
    input.min = String(Math.log10(p.min));
    input.max = String(Math.log10(p.max));
    input.step = '0.05';
    input.value = String(Math.log10(p.default));
  } else {
    input.min = String(p.min);
    input.max = String(p.max);
    input.step = String(p.step);
    input.value = String(p.default);
  }
  const value = () => (p.log ? 10 ** Number(input.value) : Number(input.value));
  const show = () => (output.textContent = format(value(), p));
  show();

  input.addEventListener('input', () => {
    show();
    if (!p.rebuild) onChange(p.key, value());
  });
  input.addEventListener('change', () => {
    if (p.rebuild) onChange(p.key, value());
  });
  label.append(row, input);
  return label;
}

function format(v: number, p: ParamSpec): string {
  if (p.log) return v.toExponential(1);
  if (p.step >= 1) return v.toFixed(0);
  const decimals = Math.min(3, Math.max(0, -Math.floor(Math.log10(p.step))));
  return v.toFixed(decimals);
}

export function formatNumber(x: number): string {
  if (!Number.isFinite(x)) return '–';
  const a = Math.abs(x);
  if (a !== 0 && (a < 1e-3 || a >= 1e5)) return x.toExponential(3);
  return x.toPrecision(4);
}
