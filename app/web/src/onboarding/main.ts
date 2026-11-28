// The walkthrough (onboarding.rs, walkthrough.rs): five screens that get a new user to a first
// dictation.
import { mount } from 'svelte';
import '../lib/ui/tokens.css';
import '../lib/ui/components.css';
import './onboarding.css';
import Onboarding from './Onboarding.svelte';

mount(Onboarding, { target: document.body });
