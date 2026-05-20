#include <ESP8266WiFi.h>
#include <ESP8266HTTPClient.h>
#include <WiFiClient.h>
#include <time.h>
#include <DHT.h>
#define WIFI_SSID "FAMILIA_CUADROS"
#define WIFI_PASSWORD "Yeyo*0207"
#define API_URL "http://your-api-endpoint.com:3000/devices/esp8266-node-01/ingest"
#define API_KEY "YOUR_API_KEY"
#define DEVICE_ID "esp8266-node-01"
#define DHTPIN 14
#define DHTTYPE DHT11
#define LDRPIN 12
#define LEDPIN 5
const unsigned long SEND_INTERVAL_MS = 30000;
const int MAX_QUEUE_SIZE = 10;
const long UTC_OFFSET_SECONDS = 0;
const int DAYLIGHT_OFFSET_SECONDS = 0;
DHT dht(DHTPIN, DHTTYPE);
WiFiClient wifiClient;
struct SensorPayload {
  String json;
};
SensorPayload queueBuffer[MAX_QUEUE_SIZE];
int queueStart = 0;
int queueCount = 0;
unsigned long lastSendTime = 0;

void flashLed() {
  digitalWrite(LEDPIN, HIGH);
  delay(100);
  digitalWrite(LEDPIN, LOW);
}

void connectWiFi() {
  if (WiFi.status() == WL_CONNECTED) {
    return;
  }
  WiFi.mode(WIFI_STA);
  WiFi.begin(WIFI_SSID, WIFI_PASSWORD);
  Serial.print("Connecting to WiFi");
  int attempts = 0;
  while (WiFi.status() != WL_CONNECTED && attempts < 20) {
    delay(500);
    Serial.print(".");
    attempts++;
  }
  Serial.println();
  if (WiFi.status() == WL_CONNECTED) {
    Serial.print("Connected. IP: ");
    Serial.println(WiFi.localIP());
  } else {
    Serial.println("WiFi connection failed");
  }
}

void syncTime() {
  configTime(UTC_OFFSET_SECONDS, DAYLIGHT_OFFSET_SECONDS, "pool.ntp.org", "time.nist.gov");
  Serial.print("Syncing time");
  time_t now = time(nullptr);
  int attempts = 0;
  while (now < 100000 && attempts < 20) {
    delay(500);
    Serial.print(".");
    now = time(nullptr);
    attempts++;
  }
  Serial.println();
  if (now >= 100000) {
    Serial.print("Time synced: ");
    Serial.println(now);
  } else {
    Serial.println("NTP sync failed");
  }
}

String getIsoTimestamp() {
  time_t now = time(nullptr);
  if (now < 100000) {
    return "";
  }
  struct tm* timeInfo = gmtime(&now);
  if (timeInfo == nullptr) {
    return "";
  }
  char timestamp[25];
  strftime(timestamp, sizeof(timestamp), "%Y-%m-%dT%H:%M:%SZ", timeInfo);
  return String(timestamp);
}

String makePayload(float temperatureC, float humidity, int lightLevel, const String& timestampIso) {
  String payload = "{";
  payload += "\"timestamp\":\"" + timestampIso + "\",";
  payload += "\"payload\":{";
  payload += "\"temperature\":" + String(temperatureC, 1) + ",";
  payload += "\"humidity\":" + String(humidity, 1) + ",";
  payload += "\"light\":" + String(lightLevel);
  payload += "}}";
  return payload;
}

bool enqueuePayload(const String& payload) {
  if (queueCount >= MAX_QUEUE_SIZE) {
    Serial.println("Queue full, dropping oldest payload");
    queueStart = (queueStart + 1) % MAX_QUEUE_SIZE;
    queueCount--;
  }
  int insertIndex = (queueStart + queueCount) % MAX_QUEUE_SIZE;
  queueBuffer[insertIndex].json = payload;
  queueCount++;
  return true;
}

bool postPayload(const String& payload) {
  if (WiFi.status() != WL_CONNECTED) {
    return false;
  }
  flashLed();
  HTTPClient http;
  http.begin(wifiClient, API_URL);
  http.addHeader("Content-Type", "application/json");
  http.addHeader("X-API-Key", API_KEY);
  int httpCode = http.POST(payload);
  Serial.print("Sending: ");
  Serial.println(payload);
  Serial.print("HTTP code: ");
  Serial.println(httpCode);
  bool success = httpCode >= 200 && httpCode < 300;
  if (httpCode > 0) {
    String response = http.getString();
    Serial.print("Response: ");
    Serial.println(response);
  } else {
    Serial.print("POST failed: ");
    Serial.println(http.errorToString(httpCode));
  }
  http.end();
  return success;
}

void flushQueue() {
  while (queueCount > 0 && WiFi.status() == WL_CONNECTED) {
    String payload = queueBuffer[queueStart].json;
    if (!postPayload(payload)) {
      Serial.println("Retry later, keeping payload in queue");
      return;
    }
    queueStart = (queueStart + 1) % MAX_QUEUE_SIZE;
    queueCount--;
    Serial.println("Queued payload sent");
  }
}

void setup() {
  Serial.begin(115200);
  delay(100);
  dht.begin();
  pinMode(LDRPIN, INPUT);
  pinMode(LEDPIN, OUTPUT);
  digitalWrite(LEDPIN, LOW);
  connectWiFi();
  syncTime();
}

void loop() {
  connectWiFi();
  if (WiFi.status() == WL_CONNECTED && getIsoTimestamp() == "") {
    syncTime();
  }
  flushQueue();
  unsigned long nowMs = millis();
  if (nowMs - lastSendTime < SEND_INTERVAL_MS) {
    delay(500);
    return;
  }
  lastSendTime = nowMs;
  float humidity = dht.readHumidity();
  float temperatureC = dht.readTemperature();
  int lightLevel = digitalRead(LDRPIN);
  if (isnan(humidity) || isnan(temperatureC)) {
    Serial.println("DHT11 read failed");
    return;
  }
  String timestampIso = getIsoTimestamp();
  if (timestampIso == "") {
    timestampIso = "1970-01-01T00:00:00Z";
  }
  String payload = makePayload(temperatureC, humidity, lightLevel, timestampIso);
  if (!postPayload(payload)) {
    Serial.println("Storing payload for retry");
    enqueuePayload(payload);
  }
}
