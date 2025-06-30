import { useState, useEffect } from 'react';
import {
  ChakraProvider,
  Box,
  Tab,
  TabList,
  TabPanel,
  TabPanels,
  Tabs,
  extendTheme,
  ColorModeScript,
} from '@chakra-ui/react';
import { BackupTab } from './components/BackupTab';
import { LogsTab } from './components/LogsTab';
import { ScheduleTab } from './components/ScheduleTab';
import { HistoryTab } from './components/HistoryTab';
import { Header } from './components/Header';
import { ControlButtons } from './components/ControlButtons';
import { useBackupStore } from './store/backupStore';
import { useWebSocket } from './hooks/useWebSocket';
import { fetchStatus } from './services/api';

// Professional grey theme
const theme = extendTheme({
  config: {
    initialColorMode: 'dark',
    useSystemColorMode: false,
  },
  styles: {
    global: {
      body: {
        bg: 'gray.900',
        color: 'gray.100',
      },
    },
  },
  colors: {
    brand: {
      50: '#E6F4EA',
      100: '#C4E5CC',
      200: '#9FD4AD',
      300: '#7AC48F',
      400: '#5BB675',
      500: '#3FA85F', // Professional green accent
      600: '#369B52',
      700: '#2C8545',
      800: '#236F38',
      900: '#1A5A2B',
    },
    gray: {
      50: '#F7F8FA',
      100: '#E9ECEF',
      200: '#DEE2E6',
      300: '#CED4DA',
      400: '#ADB5BD',
      500: '#6C757D',
      600: '#495057',
      700: '#343A40',
      800: '#212529',
      900: '#0F1114', // Darkest background
    },
    accent: {
      blue: '#4A90E2',
      green: '#3FA85F',
      orange: '#F5A623',
      red: '#DC3545',
      purple: '#6B46C1',
    },
  },
  components: {
    Button: {
      defaultProps: {
        colorScheme: 'brand',
      },
    },
    Progress: {
      defaultProps: {
        colorScheme: 'brand',
      },
    },
  },
});

function App() {
  const [activeTab, setActiveTab] = useState(0);
  const { status, setStatus } = useBackupStore();
  
  // Initialize WebSocket connection
  useWebSocket();

  // Initial status fetch
  useEffect(() => {
    fetchStatus().then(setStatus).catch(console.error);
  }, [setStatus]);

  return (
    <ChakraProvider theme={theme}>
      <ColorModeScript initialColorMode={theme.config.initialColorMode} />
      <Box minH="100vh" bg="gray.900" color="gray.50">
        <Header status={status} />
        
        <Tabs
          index={activeTab}
          onChange={setActiveTab}
          variant="unstyled"
          colorScheme="brand"
        >
          <TabList bg="gray.800" borderBottom="1px solid" borderColor="gray.600">
            <Tab
              _selected={{
                color: 'brand.500',
                borderBottom: '3px solid',
                borderColor: 'brand.500',
              }}
              _hover={{ color: 'gray.50' }}
              color="gray.300"
              px={6}
              py={3}
            >
              Backup
            </Tab>
            <Tab
              _selected={{
                color: 'brand.500',
                borderBottom: '3px solid',
                borderColor: 'brand.500',
              }}
              _hover={{ color: 'gray.50' }}
              color="gray.300"
              px={6}
              py={3}
            >
              Logs
            </Tab>
            <Tab
              _selected={{
                color: 'brand.500',
                borderBottom: '3px solid',
                borderColor: 'brand.500',
              }}
              _hover={{ color: 'gray.50' }}
              color="gray.300"
              px={6}
              py={3}
            >
              Schedule
            </Tab>
            <Tab
              _selected={{
                color: 'brand.500',
                borderBottom: '3px solid',
                borderColor: 'brand.500',
              }}
              _hover={{ color: 'gray.50' }}
              color="gray.300"
              px={6}
              py={3}
            >
              History
            </Tab>
          </TabList>

          <TabPanels>
            <TabPanel p={0}>
              <BackupTab status={status} />
            </TabPanel>
            <TabPanel p={0}>
              <LogsTab logs={status?.logs || []} />
            </TabPanel>
            <TabPanel p={0}>
              <ScheduleTab />
            </TabPanel>
            <TabPanel p={0}>
              <HistoryTab history={status?.history || []} />
            </TabPanel>
          </TabPanels>
        </Tabs>

        <ControlButtons status={status} />
      </Box>
    </ChakraProvider>
  );
}

export default App;