import React, { useState } from 'react';
import {
  Box,
  VStack,
  HStack,
  Text,
  Button,
  ButtonGroup,
  Input,
  InputGroup,
  InputLeftElement,
  useColorModeValue,
  Code,
  Badge,
} from '@chakra-ui/react';
import { FaSearch, FaDownload, FaTrash } from 'react-icons/fa';

interface LogEntry {
  timestamp: string;
  level: 'info' | 'warning' | 'error';
  message: string;
}

interface LogsTabProps {
  logs: LogEntry[];
}

export function LogsTab({ logs }: LogsTabProps) {
  const [filter, setFilter] = useState<'all' | 'errors' | 'warnings' | 'info'>('all');
  const [searchTerm, setSearchTerm] = useState('');

  const bgColor = useColorModeValue('gray.50', 'gray.800');
  const cardBg = useColorModeValue('white', 'gray.700');
  const borderColor = useColorModeValue('gray.200', 'gray.600');
  const textMuted = useColorModeValue('gray.600', 'gray.400');
  const codeBg = useColorModeValue('gray.100', 'gray.900');

  const filteredLogs = logs.filter(log => {
    if (filter !== 'all' && log.level !== filter) return false;
    if (searchTerm && !log.message.toLowerCase().includes(searchTerm.toLowerCase())) return false;
    return true;
  });

  const getLevelColor = (level: string) => {
    switch (level) {
      case 'error': return 'red';
      case 'warning': return 'yellow';
      case 'info': return 'blue';
      default: return 'gray';
    }
  };

  const downloadLogs = () => {
    const content = filteredLogs.map(log => 
      `[${log.timestamp}] ${log.level.toUpperCase()}: ${log.message}`
    ).join('\n');
    
    const blob = new Blob([content], { type: 'text/plain' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `backup-logs-${new Date().toISOString()}.txt`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
  };

  return (
    <Box p={6}>
      <VStack spacing={4} align="stretch">
        <Box>
          <Text fontSize="lg" fontWeight="bold" mb={4} color="brand.500">
            RSYNC LOGS
          </Text>
          
          <HStack spacing={4} mb={4}>
            <ButtonGroup size="sm" isAttached variant="outline">
              <Button 
                isActive={filter === 'all'}
                onClick={() => setFilter('all')}
                colorScheme={filter === 'all' ? 'brand' : 'gray'}
              >
                All
              </Button>
              <Button 
                isActive={filter === 'errors'}
                onClick={() => setFilter('errors')}
                colorScheme={filter === 'errors' ? 'brand' : 'gray'}
              >
                Errors
              </Button>
              <Button 
                isActive={filter === 'warnings'}
                onClick={() => setFilter('warnings')}
                colorScheme={filter === 'warnings' ? 'brand' : 'gray'}
              >
                Warnings
              </Button>
              <Button 
                isActive={filter === 'info'}
                onClick={() => setFilter('info')}
                colorScheme={filter === 'info' ? 'brand' : 'gray'}
              >
                Info
              </Button>
            </ButtonGroup>
            
            <Button 
              size="sm" 
              leftIcon={<FaDownload />}
              onClick={downloadLogs}
              colorScheme="brand"
            >
              Download Log File
            </Button>
          </HStack>
          
          <InputGroup size="sm" mb={4}>
            <InputLeftElement pointerEvents="none">
              <FaSearch color={textMuted} />
            </InputLeftElement>
            <Input 
              placeholder="Search logs..."
              value={searchTerm}
              onChange={(e) => setSearchTerm(e.target.value)}
              bg={cardBg}
            />
          </InputGroup>
        </Box>
        
        <Box 
          bg={codeBg} 
          p={4} 
          borderRadius="md" 
          height="500px" 
          overflowY="auto"
          fontFamily="mono"
          fontSize="sm"
        >
          {filteredLogs.length === 0 ? (
            <Text color={textMuted} textAlign="center" py={8}>
              No logs to display
            </Text>
          ) : (
            <VStack spacing={2} align="stretch">
              {filteredLogs.map((log, index) => (
                <Box 
                  key={index}
                  p={2}
                  bg={cardBg}
                  borderRadius="sm"
                  borderLeft="3px solid"
                  borderLeftColor={`${getLevelColor(log.level)}.500`}
                >
                  <HStack spacing={2} mb={1}>
                    <Badge 
                      colorScheme={getLevelColor(log.level)} 
                      size="sm"
                      variant="subtle"
                    >
                      {log.level.toUpperCase()}
                    </Badge>
                    <Text fontSize="xs" color={textMuted}>
                      {new Date(log.timestamp).toLocaleString()}
                    </Text>
                  </HStack>
                  <Code 
                    fontSize="xs" 
                    bg="transparent" 
                    color="inherit"
                    whiteSpace="pre-wrap"
                  >
                    {log.message}
                  </Code>
                </Box>
              ))}
            </VStack>
          )}
        </Box>
      </VStack>
    </Box>
  );
}