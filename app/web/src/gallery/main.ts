// Every shared control in its states, for the screenshot checks and for anyone building a page
// (docs/DESIGN.md, "Components"). Built only when HYPERME_GALLERY is set, so it never ships.
import { mount } from 'svelte';
import '../lib/ui/tokens.css';
import '../lib/ui/components.css';
import Gallery from './Gallery.svelte';

mount(Gallery, { target: document.body });
