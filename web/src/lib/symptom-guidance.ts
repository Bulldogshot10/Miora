export interface SymptomGuidance {
  title: string;
  summary: string;
  actions: string[];
  seekCare: string;
  sources: { label: string; url: string }[];
}

export const symptomGuidance: Record<string, SymptomGuidance> = {
  cramps: {
    title: "Period cramps",
    summary: "Period pain is commonly caused by the uterus contracting during menstruation.",
    actions: [
      "Place a heat pad or hot-water bottle wrapped in a towel on the lower abdomen, or try a warm bath or shower.",
      "Try gentle movement such as walking, swimming or yoga if it feels comfortable.",
      "Ibuprofen and other anti-inflammatory pain relievers can help period pain. Follow the product label and ask a pharmacist or clinician whether an anti-inflammatory is suitable for her health history and other medicines.",
    ],
    seekCare: "Get medical advice if pain is severe or worse than usual, painkillers do not help, periods become heavier or irregular, or pain affects sex, urination or bowel movements.",
    sources: [
      { label: "NHS · Period pain", url: "https://www.nhs.uk/symptoms/period-pain/" },
      { label: "NHS · Period problems", url: "https://www.nhs.uk/conditions/periods/period-problems/" },
    ],
  },
  bloating: {
    title: "Bloating",
    summary: "Bloating can occur around a period. Digestive gas, constipation and food triggers can also contribute.",
    actions: [
      "Drink water and keep up gentle regular movement; a walk can help when she feels bloated.",
      "Try smaller, more frequent meals rather than large meals.",
      "Notice whether fizzy drinks, excess caffeine or particular foods make it worse, and reduce personal triggers.",
      "If constipation is present, fibre-containing foods such as oats may help. A pharmacist can explain whether an option such as simeticone is appropriate for trapped wind.",
    ],
    seekCare: "Arrange a medical review if bloating is frequent, persistent, associated with unintentional weight loss or blood in stool, or does not improve after changes.",
    sources: [
      { label: "NHS · Bloating", url: "https://www.nhs.uk/symptoms/bloating/" },
      { label: "NHS · PMS", url: "https://www.nhs.uk/conditions/pre-menstrual-syndrome/" },
    ],
  },
  headache: {
    title: "Headache",
    summary: "Hormone changes around menstruation can be associated with headaches or menstrual migraine. A symptom diary can help reveal timing and triggers.",
    actions: [
      "Drink water, eat regular meals and avoid skipping meals.",
      "Rest, relax and take breaks from prolonged screen use if it aggravates the headache.",
      "If she has migraine-like symptoms, resting in a dark, quiet room may help.",
      "Paracetamol or ibuprofen can help some headaches. Follow the product label and ask a pharmacist if she is unsure which medicine is suitable.",
      "Record when the headache starts, how long it lasts and whether it occurs near the period; bring the pattern to a clinician if it recurs.",
    ],
    seekCare: "Seek urgent help for a sudden, extremely painful headache, weakness or numbness, trouble speaking or walking, confusion, seizure or loss of vision. Speak with a clinician about headaches that recur, worsen or are difficult to control.",
    sources: [
      { label: "NHS · Headaches", url: "https://www.nhs.uk/symptoms/headaches/" },
      { label: "NHS · Migraine", url: "https://www.nhs.uk/conditions/migraine/" },
    ],
  },
  fatigue: {
    title: "Fatigue",
    summary: "Tiredness and sleep changes are recognised premenstrual symptoms, but fatigue can also have causes unrelated to the menstrual cycle.",
    actions: [
      "Keep a regular sleep and wake schedule, including on weekends.",
      "Use gentle, regular aerobic activity such as walking, cycling or swimming; scale it to how she feels.",
      "Eat regular, balanced meals. If large meals feel uncomfortable, smaller meals more often may be easier.",
      "Track fatigue alongside period dates, sleep and bleeding so a clinician can see whether a pattern repeats.",
    ],
    seekCare: "Speak with a healthcare professional if fatigue is persistent, unusually intense, affects normal activities or occurs with very heavy bleeding. Do not automatically attribute ongoing fatigue to PMS.",
    sources: [
      { label: "ACOG · Premenstrual Syndrome", url: "https://www.acog.org/womens-health/faqs/premenstrual-syndrome" },
      { label: "NHS · PMS", url: "https://www.nhs.uk/conditions/pre-menstrual-syndrome/" },
    ],
  },
  mood_changes: {
    title: "Mood changes",
    summary: "Irritability, anxiety, low mood and mood swings can occur before a period. Repeated daily tracking helps show whether symptoms follow a cycle-related pattern.",
    actions: [
      "Protect a consistent sleep routine and include regular physical activity where possible.",
      "Try a stress-management practice she finds comfortable, such as breathing exercises, yoga or meditation.",
      "Record mood and symptom severity every day across at least two or three months, along with period dates.",
      "Cognitive behavioural therapy (CBT) is an evidence-supported treatment option for PMS. A clinician can discuss this and other treatments if symptoms are disruptive.",
    ],
    seekCare: "Talk with a clinician if mood symptoms repeatedly interfere with daily life or relationships. If she feels at risk of harming herself or cannot stay safe, seek urgent local mental-health or emergency support.",
    sources: [
      { label: "RCOG · Managing PMS", url: "https://www.rcog.org.uk/for-the-public/browse-our-patient-information/managing-premenstrual-syndrome-pms/" },
      { label: "ACOG · Premenstrual Syndrome", url: "https://www.acog.org/womens-health/faqs/premenstrual-syndrome" },
      { label: "NHS · PMS", url: "https://www.nhs.uk/conditions/pre-menstrual-syndrome/" },
    ],
  },
};
