// Strings for 0.5: the compact dashboard. Merged into STRINGS when imported (home.js imports it).
import { STRINGS } from './i18n.js';

const EN5 = {
  'home.sub5': '{n} nodes · {t} templates · everything stays on this computer',
  'home.new5': 'New',
  'home.open5': 'Open',
  'home.continue5': 'Editor',
  'home.continue5_t': 'Continue editing: {name}',
  'home.templates5': 'Templates',
  'home.recent5': 'Recent runs',
  'home.no_runs5': 'No runs yet. Pick a template or open the Converter.',
  'home.runs5': 'Runs',
  'home.files5': 'Files saved',
  'home.success5': 'Success',
  'home.time5': 'Time',
  'home.days5': '14 days',
};
const RU5 = {
  'home.sub5': '{n} нод · {t} шаблонов · всё остаётся на этом компьютере',
  'home.new5': 'Новый',
  'home.open5': 'Открыть',
  'home.continue5': 'Редактор',
  'home.continue5_t': 'Продолжить: {name}',
  'home.templates5': 'Шаблоны',
  'home.recent5': 'Последние запуски',
  'home.no_runs5': 'Запусков пока нет. Выберите шаблон или откройте конвертер.',
  'home.runs5': 'Запуски',
  'home.files5': 'Файлов',
  'home.success5': 'Успешно',
  'home.time5': 'Время',
  'home.days5': '14 дней',
};
Object.assign(STRINGS.en, EN5);
Object.assign(STRINGS.ru, RU5);
