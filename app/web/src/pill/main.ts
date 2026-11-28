// The pill's page (docs/UX.md, "The pill"): the capsule, the voice and the morning card.
import { mount } from 'svelte';
import '../lib/ui/tokens.css';
import '../lib/ui/components.css';
import './pill.css';
import Pill from './Pill.svelte';

mount(Pill, { target: document.body });
