import '@nigel/theme/css/nigel.css';
import { loadPreviews } from './manifest.js';
import './app/preview-app.js';

const previews = loadPreviews();
const app = document.createElement('preview-app');
(app as HTMLElement & { previews: typeof previews }).previews = previews;
document.body.appendChild(app);
