import type { TemplateResult } from 'lit';

export type PreviewBackground = 'default' | 'surface' | 'inverse' | 'transparent';

export type PreviewLayout = 'grid' | 'grid-wide' | 'stack';

export interface PreviewState {
  name: string;
  render: () => TemplateResult;
  background?: PreviewBackground;
}

export interface Preview {
  id: string;
  title: string;
  group: string;
  description?: string;
  layout?: PreviewLayout;
  states: PreviewState[];
}
